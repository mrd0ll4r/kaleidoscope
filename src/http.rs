use crate::runtime::runtime::Runtime;
use alloy::config::UniverseConfig;
use anyhow::Context;
use anyhow::Result;
use std::fmt;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::Mutex;
use warp::Filter;

/// Wrapper to pretty-print optional values.
struct OptFmt<T>(Option<T>);

impl<T: fmt::Display> fmt::Display for OptFmt<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(ref t) = self.0 {
            fmt::Display::fmt(t, f)
        } else {
            f.write_str("-")
        }
    }
}

pub(crate) async fn run_server(
    addr: SocketAddr,
    state: Arc<Mutex<Runtime>>,
    universe: Arc<UniverseConfig>,
) -> Result<()> {
    let api = filters::docs().or(filters::api(state, universe));

    let routes = api.with(warp::log::custom(move |info: warp::log::Info<'_>| {
        // This is the exact same as warp::log::log("api"), but logging at DEBUG instead of INFO.
        log::debug!(
            target: "api",
            "{} \"{} {} {:?}\" {} \"{}\" \"{}\" {:?}",
            OptFmt(info.remote_addr()),
            info.method(),
            info.path(),
            info.version(),
            info.status().as_u16(),
            OptFmt(info.referer()),
            OptFmt(info.user_agent()),
            info.elapsed(),
        );
    }));

    // Start up the server...
    let (_, fut) = warp::serve(routes)
        .try_bind_ephemeral(addr)
        .context("unable to bind")?;
    fut.await;

    Ok(())
}

mod filters {
    use super::handlers;
    use crate::runtime::runtime::Runtime;
    use alloy::config::UniverseConfig;
    use alloy::program::ParameterSetRequest;
    use futures::future;
    use log::warn;
    use std::sync::Arc;
    use tokio::sync::Mutex;
    use warp::hyper::body::Bytes;
    use warp::{body, path, Filter};

    // TODO add route to cycle active program per fixture
    // TODO add route to cycle discrete parameter value
    // TODO add route to set multiple parameters at once
    // TODO add route to enable/disable/cycle multiple programs at one

    pub(crate) fn docs(
    ) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
        path::end().map(|| {
            let routes = vec![
                "GET  /api/v1/fixtures                                                        List fixtures.",
                "GET  /api/v1/fixtures/:fixture                                               Get single fixture.",
                "GET  /api/v1/fixtures/:fixture/programs                                      List programs for fixture.",
                "POST /api/v1/fixtures/:fixture/set_active_program                            Set active program by name, provide the name as text in the body.",
                "POST /api/v1/fixtures/:fixture/cycle_active_program                          Cycle to the next program, skipping MANUAL and EXTERNAL.",
                "GET  /api/v1/fixtures/:fixture/programs/:program                             Get single program.",
                "GET  /api/v1/fixtures/:fixture/programs/:program/parameters                  List parameters for program.",
                "GET  /api/v1/fixtures/:fixture/programs/:program/parameters/:parameter       Get single parameter.",
                "POST /api/v1/fixtures/:fixture/programs/:program/parameters/:parameter       Set parameter value, provide an alloy::program::ParameterSetRequest as JSON in the body.",
                "POST /api/v1/fixtures/:fixture/programs/:program/parameters/:parameter/cycle Cycle discrete parameter value.",
                "" // For newline at the end
            ];
            routes.join("\n")
        })
    }

    pub(crate) fn api(
        state: Arc<Mutex<Runtime>>,
        universe: Arc<UniverseConfig>,
    ) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
        warp::path!("api" / "v1" / ..).and(
            fixtures_root(state.clone(), universe.clone())
                .or(fixtures_fixture_root(state.clone(), universe.clone()))
                .or(fixtures_fixture_programs_set_active(state.clone()))
                .or(fixtures_fixture_programs_cycle_active(state.clone()))
                .or(fixtures_fixture_programs_root(
                    state.clone(),
                    universe.clone(),
                ))
                .or(fixtures_fixture_programs_program_root(state.clone()))
                .or(fixtures_fixture_programs_program_parameters_root(
                    state.clone(),
                ))
                .or(fixtures_fixture_programs_program_parameters_parameter_get(
                    state.clone(),
                ))
                .or(fixtures_fixture_programs_program_parameters_parameter_set(
                    state.clone(),
                ))
                .or(fixtures_fixture_programs_program_parameters_parameter_cycle(state.clone())),
        )
    }

    pub(crate) fn fixtures_root(
        state: Arc<Mutex<Runtime>>,
        universe: Arc<UniverseConfig>,
    ) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
        warp::path!("fixtures")
            .and(path::end())
            .and(warp::get())
            .and(with_state(state))
            .and(with_universe_config(universe))
            .and_then(handlers::get_fixtures_root)
    }

    pub(crate) fn fixtures_fixture_root(
        state: Arc<Mutex<Runtime>>,
        universe: Arc<UniverseConfig>,
    ) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
        warp::path!("fixtures" / String)
            .and(path::end())
            .and(warp::get())
            .and(with_state(state))
            .and(with_universe_config(universe))
            .and_then(handlers::get_fixtures_fixture_root)
    }

    pub(crate) fn fixtures_fixture_programs_root(
        state: Arc<Mutex<Runtime>>,
        universe: Arc<UniverseConfig>,
    ) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
        warp::path!("fixtures" / String / "programs")
            .and(path::end())
            .and(warp::get())
            .and(with_state(state))
            .and(with_universe_config(universe))
            .and_then(handlers::get_fixtures_fixture_programs_root)
    }

    pub(crate) fn fixtures_fixture_programs_set_active(
        state: Arc<Mutex<Runtime>>,
    ) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
        warp::path!("fixtures" / String / "set_active_program")
            .and(path::end())
            .and(warp::post())
            .and(set_active_program_body())
            .and(with_state(state))
            .and_then(handlers::post_fixtures_fixture_set_program)
    }

    pub(crate) fn fixtures_fixture_programs_cycle_active(
        state: Arc<Mutex<Runtime>>,
    ) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
        warp::path!("fixtures" / String / "cycle_active_program")
            .and(path::end())
            .and(warp::post())
            .and(with_state(state))
            .and_then(handlers::post_fixtures_fixture_cycle_program)
    }

    pub(crate) fn fixtures_fixture_programs_program_root(
        state: Arc<Mutex<Runtime>>,
    ) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
        warp::path!("fixtures" / String / "programs" / String)
            .and(path::end())
            .and(warp::get())
            .and(with_state(state))
            .and_then(handlers::get_fixtures_fixture_programs_program_root)
    }

    pub(crate) fn fixtures_fixture_programs_program_parameters_root(
        state: Arc<Mutex<Runtime>>,
    ) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
        warp::path!("fixtures" / String / "programs" / String / "parameters")
            .and(path::end())
            .and(warp::get())
            .and(with_state(state))
            .and_then(handlers::get_fixtures_fixture_programs_program_parameters_root)
    }

    pub(crate) fn fixtures_fixture_programs_program_parameters_parameter_get(
        state: Arc<Mutex<Runtime>>,
    ) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
        warp::path!("fixtures" / String / "programs" / String / "parameters" / String)
            .and(path::end())
            .and(warp::get())
            .and(with_state(state))
            .and_then(handlers::get_fixtures_fixture_programs_program_parameters_parameter)
    }

    pub(crate) fn fixtures_fixture_programs_program_parameters_parameter_set(
        state: Arc<Mutex<Runtime>>,
    ) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
        warp::path!("fixtures" / String / "programs" / String / "parameters" / String)
            .and(path::end())
            .and(warp::post())
            .and(with_state(state))
            .and(parameter_request_body())
            .and_then(handlers::post_fixtures_fixture_programs_program_parameters_parameter)
    }

    pub(crate) fn fixtures_fixture_programs_program_parameters_parameter_cycle(
        state: Arc<Mutex<Runtime>>,
    ) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
        warp::path!("fixtures" / String / "programs" / String / "parameters" / String / "cycle")
            .and(path::end())
            .and(warp::post())
            .and(with_state(state))
            .and_then(handlers::post_fixtures_fixture_programs_program_parameters_parameter_cycle)
    }

    fn with_state(
        state: Arc<Mutex<Runtime>>,
    ) -> impl Filter<Extract = (Arc<Mutex<Runtime>>,), Error = std::convert::Infallible> + Clone
    {
        warp::any().map(move || state.clone())
    }

    fn with_universe_config(
        universe: Arc<UniverseConfig>,
    ) -> impl Filter<Extract = (Arc<UniverseConfig>,), Error = std::convert::Infallible> + Clone
    {
        warp::any().map(move || universe.clone())
    }

    fn parameter_request_body(
    ) -> impl Filter<Extract = (ParameterSetRequest,), Error = warp::Rejection> + Clone {
        // When accepting a body, we want a JSON body
        // (and to reject huge payloads)...
        body::content_length_limit(1024).and(body::json())
    }

    fn set_active_program_body() -> impl Filter<Extract = (String,), Error = warp::Rejection> + Clone
    {
        body::content_length_limit(1024)
            .and(body::bytes())
            .and_then(|b: Bytes| match String::from_utf8(b.to_vec()) {
                Ok(s) => future::ok(s),
                Err(_) => {
                    warn!("non-utf8 bytes supplied to set_active_program_body");
                    future::err(warp::reject::custom(NonUtf8Body))
                }
            })
    }

    #[derive(Debug)]
    struct NonUtf8Body;

    impl warp::reject::Reject for NonUtf8Body {}
}

mod handlers {
    use crate::runtime::runtime::Runtime;
    use alloy::config::UniverseConfig;
    use alloy::program::ParameterSetRequest;
    use log::debug;
    use std::convert::Infallible;
    use std::sync::Arc;
    use tokio::sync::Mutex;
    use warp::{http, Rejection};

    pub(crate) async fn get_fixtures_root(
        state: Arc<Mutex<Runtime>>,
        universe: Arc<UniverseConfig>,
    ) -> Result<impl warp::Reply, Infallible> {
        let cfg = state.lock().await.alloy_metadata(universe.as_ref());

        Ok(warp::reply::json(&cfg))
    }

    pub(crate) async fn get_fixtures_fixture_root(
        fixture_name: String,
        state: Arc<Mutex<Runtime>>,
        universe: Arc<UniverseConfig>,
    ) -> Result<impl warp::Reply, Rejection> {
        if let Some(fixture) = state.lock().await.get_fixture(&fixture_name) {
            Ok(warp::reply::json(
                &fixture.alloy_metadata(universe.as_ref()),
            ))
        } else {
            Err(warp::reject::not_found())
        }
    }

    pub(crate) async fn get_fixtures_fixture_programs_root(
        fixture_name: String,
        state: Arc<Mutex<Runtime>>,
        universe: Arc<UniverseConfig>,
    ) -> Result<impl warp::Reply, Rejection> {
        if let Some(fixture) = state.lock().await.get_fixture(&fixture_name) {
            Ok(warp::reply::json(
                &fixture.alloy_metadata(universe.as_ref()),
            ))
        } else {
            Err(warp::reject::not_found())
        }
    }

    pub(crate) async fn post_fixtures_fixture_set_program(
        fixture_name: String,
        program_name: String,
        state: Arc<Mutex<Runtime>>,
    ) -> Result<impl warp::Reply, Rejection> {
        let mut state = state.lock().await;

        if let Some(fixture) = state.get_fixture_mut(&fixture_name) {
            let res = fixture.set_active_program(&program_name);
            debug!("fixture::set_active_program returned {:?}", res);
            // TODO figure out proper errors
            match res {
                Ok(_) => Ok(http::StatusCode::OK),
                Err(_) => Ok(http::StatusCode::NOT_FOUND),
            }
        } else {
            Err(warp::reject::not_found())
        }
    }

    pub(crate) async fn post_fixtures_fixture_cycle_program(
        fixture_name: String,
        state: Arc<Mutex<Runtime>>,
    ) -> Result<impl warp::Reply, Rejection> {
        let mut state = state.lock().await;

        if let Some(fixture) = state.get_fixture_mut(&fixture_name) {
            let res = fixture.cycle_active_program();
            debug!("fixture::cycle_active_program returned {:?}", res);
            // TODO figure out proper errors
            match res {
                Ok(new_program) => Ok(warp::reply::json(&new_program)),
                Err(_) => Err(warp::reject::not_found()),
            }
        } else {
            Err(warp::reject::not_found())
        }
    }

    pub(crate) async fn get_fixtures_fixture_programs_program_root(
        fixture_name: String,
        program_name: String,
        state: Arc<Mutex<Runtime>>,
    ) -> Result<impl warp::Reply, Rejection> {
        if let Some(fixture) = state.lock().await.get_fixture(&fixture_name) {
            if let Some(program) = fixture.get_program(&program_name) {
                Ok(warp::reply::json(&program.alloy_metadata()))
            } else {
                Err(warp::reject::not_found())
            }
        } else {
            Err(warp::reject::not_found())
        }
    }

    pub(crate) async fn get_fixtures_fixture_programs_program_parameters_root(
        fixture_name: String,
        program_name: String,
        state: Arc<Mutex<Runtime>>,
    ) -> Result<impl warp::Reply, Rejection> {
        if let Some(fixture) = state.lock().await.get_fixture(&fixture_name) {
            if let Some(program) = fixture.get_program(&program_name) {
                Ok(warp::reply::json(&program.alloy_metadata()))
            } else {
                Err(warp::reject::not_found())
            }
        } else {
            Err(warp::reject::not_found())
        }
    }

    pub(crate) async fn get_fixtures_fixture_programs_program_parameters_parameter(
        fixture_name: String,
        program_name: String,
        parameter_name: String,
        state: Arc<Mutex<Runtime>>,
    ) -> Result<impl warp::Reply, Rejection> {
        if let Some(fixture) = state.lock().await.get_fixture(&fixture_name) {
            if let Some(program) = fixture.get_program(&program_name) {
                if let Some(parameter) = program.get_parameter(&parameter_name) {
                    Ok(warp::reply::json(&parameter.alloy_metadata()))
                } else {
                    Err(warp::reject::not_found())
                }
            } else {
                Err(warp::reject::not_found())
            }
        } else {
            Err(warp::reject::not_found())
        }
    }

    pub(crate) async fn post_fixtures_fixture_programs_program_parameters_parameter(
        fixture_name: String,
        program_name: String,
        parameter_name: String,
        state: Arc<Mutex<Runtime>>,
        set_request: ParameterSetRequest,
    ) -> Result<impl warp::Reply, Rejection> {
        let mut state = state.lock().await;

        if let Some(fixture) = state.get_fixture_mut(&fixture_name) {
            if let Some(program) = fixture.get_program_mut(&program_name) {
                if let Some(parameter) = program.get_parameter_mut(&parameter_name) {
                    let res = parameter.set(set_request);
                    debug!("parameter::set returned {:?}", res);
                    // TODO figure out proper errors
                    match res {
                        Ok(_) => Ok(http::StatusCode::OK),
                        Err(_) => Ok(http::StatusCode::BAD_REQUEST),
                    }
                } else {
                    Err(warp::reject::not_found())
                }
            } else {
                Err(warp::reject::not_found())
            }
        } else {
            Err(warp::reject::not_found())
        }
    }

    pub(crate) async fn post_fixtures_fixture_programs_program_parameters_parameter_cycle(
        fixture_name: String,
        program_name: String,
        parameter_name: String,
        state: Arc<Mutex<Runtime>>,
    ) -> Result<impl warp::Reply, Rejection> {
        let mut state = state.lock().await;

        if let Some(fixture) = state.get_fixture_mut(&fixture_name) {
            if let Some(program) = fixture.get_program_mut(&program_name) {
                if let Some(parameter) = program.get_parameter_mut(&parameter_name) {
                    let res = parameter.cycle();
                    debug!("parameter::cycle returned {:?}", res);
                    // TODO figure out proper errors
                    match res {
                        Ok(new_level) => Ok(warp::reply::json(&new_level)),
                        Err(_) => Err(warp::reject::not_found()),
                    }
                } else {
                    Err(warp::reject::not_found())
                }
            } else {
                Err(warp::reject::not_found())
            }
        } else {
            Err(warp::reject::not_found())
        }
    }
}
