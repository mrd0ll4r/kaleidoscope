# Kaleidoscope

An environment to run Lua programs on top of Submarine.

## Description

This is a program that connects to Submarine and executes Lua programs to control Output Devices attached to
the Submarine instance.

## Configuration

Configuration is done via YAML files.
The main configuration file specifies basic properties and a path to a directory containing configuration files for
the individual fixtures:
```yaml
# Address of the AMQP server used to publish status updates.
amqp_server_address: "amqp://192.168.88.30:5672/%2f"
# Address of the Submarine instance to post outputs to.
submarine_http_url: "http://192.168.88.30:3069"
# The address to expose Prometheus metrics on.
prometheus_listen_address: "0.0.0.0:4343"
# The address to expose the HTTP API on.
http_listen_address: "0.0.0.0:3545"
# The path from which to load fixtures and programs.
fixtures_path: "./fixtures"
```

## HTTP API

Kaleidoscope is controlled via a JSON-over-HTTP API.
Currently, these routes are exposed:
```
GET  /api/v1/fixtures                                                        List fixtures.
GET  /api/v1/fixtures/:fixture                                               Get single fixture.
GET  /api/v1/fixtures/:fixture/programs                                      List programs for fixture.
POST /api/v1/fixtures/:fixture/set_active_program                            Set active program by name, provide the name as text in the body.
POST /api/v1/fixtures/:fixture/cycle_active_program                          Cycle to the next program, skipping MANUAL and EXTERNAL.
GET  /api/v1/fixtures/:fixture/programs/:program                             Get single program.
GET  /api/v1/fixtures/:fixture/programs/:program/parameters                  List parameters for program.
GET  /api/v1/fixtures/:fixture/programs/:program/parameters/:parameter       Get single parameter.
POST /api/v1/fixtures/:fixture/programs/:program/parameters/:parameter       Set parameter value, provide an alloy::program::ParameterSetRequest as JSON in the body.
POST /api/v1/fixtures/:fixture/programs/:program/parameters/:parameter/cycle Cycle discrete parameter value.
```

## The Lua Runtime

We use [mlua](https://crates.io/crates/mlua), which means that our programs are Lua 5.4.

On a high level, the `Runtime` manages a list of `Fixtures`.
Each `Fixture` has a list of `Program`s, of which one is currently active and being executed.
Each `Program` can have a set of discrete and/or continuous `Parameters`.

The Runtime evaluates all Fixtures in parallel.
Each round of executions is called a tick, of which we target `200/s`.

At the end of each tick, all outputs are aggregated into one request and sent to Submarine.
Submarine then processes that request mostly-atomically.

### Fixtures

A Fixture is a set of output addresses controlled by one active Program.
It defines a list of outputs and Programs, and configures other Fixture-wide parameters.
A Fixture definition is itself a valid Lua program.
One Fixture is defined per input file, with the `setup` function setting up the Fixture.

Here's an annotated example:
```lua
SOURCE_VERSION=3

function setup()
    -- Set a name for the Fixture.
    fixture_name("klo_rgbw")

    -- Add outputs.
    add_output_alias('klo-r')
    add_output_alias('klo-g')
    add_output_alias('klo-b')
    add_output_alias('klo-w')

    -- Whether to disable the builtin MANUAL program.
    --disable_manual_program(true)
    
    -- Whether to disable the builtin ON and OFF programs.
    --disable_builtin_programs(true)
    
    -- (Optional) programs to load.
    add_program("noise", "foo/noise.lua")
end
```

The functions available for Fixture setup are listed in [src/runtime/lua/fixture_builtin.lua](src/runtime/lua/fixture_builtin.lua).

### Builtin Programs

By default, each Fixture has three programs generated for it:
- `OFF`, which sets all outputs of the fixture to `LOW`.
- `ON`, which sets all outputs of the fixture to `ON`.
- `MANUAL`, which generates a continuous parameter for each output of the fixture and sets them according to the
  parameter values.

### Programs

Every Program must contain a `setup` function and a `tick` function.
The `setup` function is called once, during Program initialization.
The `tick` function is called in regular intervals if the program is currently active.

During `setup`, a Program defines Parameters, which are mutable through the HTTP API.
Parameter values can then be accessed during the `tick` function.

In the context of `setup()`, a bunch of special functions can be called, which are not available later.
See [src/runtime/lua/program_builtin.lua](src/runtime/lua/program_builtin.lua) for a list.

#### The `tick` Function

At the heart of every program is the `tick(now: f64)` function.
It takes one parameter, the current time in `f64` seconds since an unspecified epoch available as `START`.

For enabled Programs, the Runtime usually calls this function on each tick.
Programs can elect to be handled in "slow mode", which can be useful for outputs that do not require short reaction
times or frequent updates.

Because of this, the `tick` function __must not rely on it being called in a regular interval__.
As an example: Do not increment a counter on each tick and calculate outputs based on it -- use the provided timestamp
to calculate outputs.

The `tick` function can call other functions and do whatever Lua can do, but it should run as fast as possible.
The Runtime keeps track of both the global tick duration and `tick` durations for each program, which might be useful
for debugging.

#### Slow mode

Usually programs are run at every tick.
Slow mode programs are run every 1000 ticks.
The reason for this is that some programs can probably deal with the added
latency, which frees some performance for the programs that need to execute
every tick.

#### Builtins

The Runtime provides a bunch of builtin functions and constants, of which some are written in Rust and some in Lua.
See [src/runtime/lua/program_builtin.lua](src/runtime/lua/program_builtin.lua) for a complete list.
Notable mentions:

- `KALEIDOSCOPE_VERSION: int`, which denotes the version of the Runtime.
- `START: f64` and `NOW: f64` denote the program epoch and current timestamp, both as `f64` seconds.
- `noise2d(f64, f64) -> f64` computes 2D Perlin noise in `[-1,1]`.
    This is implemented in Rust and relatively fast.
- `noise3d(f64, f64, f64) -> f64` computes 3D Perlin noise in `[-1,1]`.
    This is implemented in Rust and slower than the 2D version.
- `noise4d(f64, f64, f64, f64) -> f64` computes 4D Perlin noise in `[-1,1]`.
    This is implemented in Rust and slower than the 3D version.
- `now() -> f64` gets the time in seconds since the program epoch.
- `get_parameter_value(name)` gets the current value of the named parameter.
- `clamp(from: numer, to: number, x: number) -> number` clamps `x` to `[from, to]`. 
- `lerp(from: number, to: number, x: number) -> number` interpolates between `from` and `to`.
- `map_range(a_lower: number, a_upper: number, b_lower: number, b_upper: number, x: number) -> number` maps `x` from the
    first range to the second.
- `map_to_value(from: number, to: number, x: number) -> u16` maps `x` from `[from,to]` to the 16-bit Submarine value
    range.
- `output_alias_to_address(alias: string) -> u16` translates an alias to a numerical address, if it exists.
    Raises an error otherwise.
- `set_alias(alias: string, value: u16)` sets the output at `alias` to `value`.
    Make sure to call this with integers, probably breaks with non-integers...

## Example Programs

Here's an example program that sets four channels of an `RGBW` output to a Perlin-noise color:

```lua
SOURCE_VERSION=3

-- Constants
local r = 0
local g = 1
local b = 2
local w = 3
local sine_speed = 0.07
local noise_speed = 0.1

-- Parameters
local MODE_BRIGHTNESS_NAME = "brightness"
local MODE_BRIGHTNESS_NIGHT = "night"
local MODE_BRIGHTNESS_DAY = "day"

-- Variables
local current_brightness_mode = MODE_BRIGHTNESS_NIGHT

function setup()
    local p_brightness = new_discrete_parameter(MODE_BRIGHTNESS_NAME)
    add_discrete_parameter_level(p_brightness, MODE_BRIGHTNESS_NIGHT, "dunkel")
    add_discrete_parameter_level(p_brightness, MODE_BRIGHTNESS_DAY, "hell")
    declare_discrete_parameter(p_brightness)
end

function compute_white(index, now)
    local t = now * sine_speed

    if current_brightness_mode == MODE_BRIGHTNESS_DAY then
        return map_to_value(0, 1, map_range(-1, 1, 0.9, 1, math.sin(t + (math.pi / 4) * index)))
    end
    return map_to_value(0, 1, map_range(-1, 1, 0.7, 0.8, math.sin(t + (math.pi / 4) * index)))
end

function compute_color(index, now)
    local t = now * noise_speed

    if current_brightness_mode == MODE_BRIGHTNESS_DAY then
        return map_to_value(0, 1, map_range(-1, 1, 0.8, 1, noise2d(index, t)))
    end
    return map_to_value(0, 1, map_range(-1, 1, 0.5, 0.9, noise2d(index, t)))
end

function tick(now)
    current_brightness_mode = get_parameter_value(MODE_BRIGHTNESS_NAME)

    set_alias('klo-w', compute_white(w, now))
    set_alias('klo-r', compute_color(r, now))
    set_alias('klo-g', compute_color(g, now))
    set_alias('klo-b', compute_color(b, now))
end
```

## Compilation & Running

See the [README of Submarine](../submarine/README.md), which explains setup and cross-compilation for Linux on a Raspberry Pi.

In general, while it is not required to run Kaleidoscope on the same machine as Submarine,
we have observed that this benefits lighting performance because of lower latency variance.

