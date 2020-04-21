SOURCE_VERSION = 1

-- index constants
local pir_motion = 0
local radar_motion = 1
local button0 = 2
local button1 = 3
-- some speed
local fade_duration_secs = 0.25

-- tables storing all kinds of stuff
local starts = {}
local bases = {}
local targets = {}

-- We need these addresses if multiplexing events through one function.
local pir_motion_address = -1
local radar_motion_address = -1
local button0_address = -1
local button1_address = -1

function setup()
    set_priority(5)

    -- We subscribe to these events by alias, type name, and a handler function.
    -- The handler function gets the address for which the event was produced as well as its type and, if available,
    -- the numerical content of the event. If the event has no content, -1 is provided.
    -- This means in practice that we can multiplex all events through one function (switching by address),
    -- or handle them in different functions:
    --[[
    add_event_subscription('pir-motion', EVENT_TYPE_CHANGE, 'handle_pir_motion_event')
    add_event_subscription('radar-motion', EVENT_TYPE_CHANGE, 'handle_radar_motion_event')
    add_event_subscription('button0', EVENT_TYPE_CHANGE, 'handle_button0_event')
    add_event_subscription('button1', EVENT_TYPE_CHANGE, 'handle_button1_event')
    ]] --
    -- or:
    add_event_subscription('pir-motion', EVENT_TYPE_CHANGE, 'handle_change_event')
    add_event_subscription('radar-motion', EVENT_TYPE_CHANGE, 'handle_change_event')
    add_event_subscription('button0', EVENT_TYPE_CHANGE, 'handle_change_event')
    add_event_subscription('button1', EVENT_TYPE_CHANGE, 'handle_change_event')


    -- We set the initial targets to the current values of these inputs (i.e. HIGH or LOW in this case).
    -- We do this to have the right state when the program is initially loaded.
    -- This is actually racy, but not that much...
    targets[pir_motion] = get_alias('pir-motion')
    targets[radar_motion] = get_alias('radar-motion')
    targets[button0] = get_alias('button0')
    targets[button1] = get_alias('button1')

    for i, index in ipairs({ pir_motion, radar_motion, button0, button1 }) do
        -- we set the base to whatever
        bases[index] = 0
        -- and the start time as well
        starts[index] = START
    end

    -- Resolve these aliases.
    pir_motion_address = alias_to_address('pir-motion')
    radar_motion_address = alias_to_address('radar-motion')
    button0_address = alias_to_address('button0')
    button1_address = alias_to_address('button1')

    -- We will control this LED strip.
    add_output_group('strip3')
end

-- handle_change is a convenience method because we basically want to react the same way to every event.
function handle_change(index, to)
    local now = now()
    bases[index] = calculate(index, now)
    targets[index] = to
    starts[index] = now
end

-- These are the actual event handlers.
function handle_pir_motion_event(address, typ, new_value)
    handle_change(pir_motion, new_value)
end

function handle_radar_motion_event(address, typ, new_value)
    handle_change(radar_motion, new_value)
end

function handle_button0_event(address, typ, new_value)
    handle_change(button0, new_value)
end

function handle_button1_event(address, typ, new_value)
    handle_change(button1, new_value)
end

-- Or use one function for all of them, switching by address
function handle_change_event(address, typ, new_value)
    if address == pir_motion_address then
        handle_change(pir_motion, new_value)
    elseif address == radar_motion_address then
        handle_change(radar_motion, new_value)
    elseif address == button0_address then
        handle_change(button0, new_value)
    elseif address == button1_address then
        handle_change(button1, new_value)
    end
end

-- calculate calculates a linear interpolation between base and target based on time passed
function calculate(index, now)
    local t = (now - starts[index]) / fade_duration_secs
    t = clamp(0, 1, t)

    return lerp(bases[index], targets[index], t) + 0
end

function tick(now)
    set_alias('strip3-r', calculate(pir_motion, now))
    set_alias('strip3-g', calculate(radar_motion, now))
    set_alias('strip3-b', calculate(button0, now))
    set_alias('strip3-w', calculate(button1, now))
end
