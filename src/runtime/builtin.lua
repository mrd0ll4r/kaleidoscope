LOW = 0
HIGH = 65535
-- The name of the program file being executed, without the .lua extension.
PROGRAM_NAME = 'some-name'

-- This is set for each execution and represents a point in time when the program was started, as seconds from some point in time.
START = 123.45
-- This is set before handling events.
NOW = 123.45
-- This is set before handling events.
-- Contains the time of day in seconds since midnight.
-- This example value is 14:36:12.
TIME_OF_DAY = 14*60*60 + 36*60 + 12

-- now returns the time in seconds since the program epoch.
function now()
    return NOW
end

-- ================ SETUP ================
-- These are provided by the runtime during setup
function set_priority(p) end
function set_slow_mode(slow_mode) end
function add_input_alias(alias) end
function add_output_alias(alias) end
function add_output_group(group) end
function add_event_subscription(alias, event_type, target_function_name) end

-- Functions to control whether tick() should run.
-- For the time being, program_enable and program_disable achieve the same.
function enable_tick() end
function disable_tick() end

-- Functions to declare parameters
function build_discrete_parameter_value(label, value)
    local tmp={}
    tmp['_label'] = label
    tmp['_value'] = value
    return tmp
end

-- Parameter type constants, as used in program.rs
PARAMETER_TYPE_DISCRETE = "discrete"
PARAMETER_TYPE_CONTINUOUS = "continuous"
-- The runtime provides a function with this signature during setup:
-- function _declare_parameter_generic(param_type, param_name, description, event_handler, discrete_values, discrete_initial, continuous_lower, continuous_upper, continuous_initial) end
function declare_discrete_parameter(name, description, values, initial_value, event_handler)
    -- TODO typecheck parameters
    _declare_parameter_generic(PARAMETER_TYPE_DISCRETE, name, description, event_handler, values, initial_value, 0,0,0)
end
function declare_continuous_parameter(name, description, lower, upper, initial_value, event_handler)
    -- TODO typecheck parameters
    _declare_parameter_generic(PARAMETER_TYPE_CONTINUOUS, name, description, event_handler, {}, 0, lower, upper, initial_value)
end

-- ================ RUNTIME ================
-- Access to parameters.
function get_discrete_parameter_value(name)
    get_foreign_discrete_parameter_value(PROGRAM_NAME, name)
end
function get_continuous_parameter_value(name)
    get_foreign_continuous_parameter_value(PROGRAM_NAME, name)
end
function set_discrete_parameter_value(name, value)
    set_foreign_discrete_parameter_value(PROGRAM_NAME, name, value)
end
function set_continuous_parameter_value(name, value)
    set_foreign_continuous_parameter_value(PROGRAM_NAME, name, value)
end
function increment_discrete_parameter_value(name, delta)
    increment_foreign_discrete_parameter(PROGRAM_NAME, name, delta)
end

-- Access to parameters of other programs.
-- These call into Rust and are not cheap.
-- It should be preferred to use event handlers to receive updates about parameters.
function get_foreign_discrete_parameter_value(program_name, parameter_name) end
function get_foreign_continuous_parameter_value(program_name, parameter_name) end
function set_foreign_discrete_parameter_value(program_name, parameter_name, value) end
function set_foreign_continuous_parameter_value(program_name, parameter_name, value) end
function increment_foreign_discrete_parameter_value(program_name, parameter_name, delta) end

_parameter_event_handlers = {}
-- This is called by the runtime to handle updates to parameters.
-- Parameter updates are passed into Lua as one string. (Extensive research has shown this to be most performant...)
-- Within that string, single updates are separated by a ";".
-- Within one update, fields are separated by a space.
function _handle_parameter_events(events)
    for event in string.gmatch(events, "[^;]*") do
        --print(event)
        local split = {}
        for i in event:gmatch("%S*") do
            split[#split + 1] = i
        end
        local param_name = split[1]
        local typ = split[2]
        local val = 0
        if typ == "d" then
            -- TODO figure out integers
            val = tonumber(split[3])
        elseif typ == "c" then
            val = tonumber(split[3])
        end

        local handler = _parameter_event_handlers[param_name]
        if handler == nil then
            return
        end
        _G[handler](val)
    end
end

-- Perlin noise functions, returning values in [-1,1]
function noise2d(x, y) return 0.0 end
function noise3d(x, y, z) return 0.0 end
function noise3d(x, y, z, t) return 0.0 end

-- Functions to access and modify global, shared values.
_globals = {}
function get_global(key)
    return _globals[key]
end

-- The deltas to global values are picked up before processing events, from all
-- loaded programs.
-- They are then aggregated and redistributed to all loaded programs, before
-- processing events.
-- This is probably expensive. Don't set globals too often, I guess...
-- If multiple programs update the same global during the same tick it's
-- unspecified which value will be propagated for the next tick.
-- In particular, this can lead to inconsistent state between programs.
_global_deltas = {}
function set_global(key, value)
    assert(type(key) == "string", "global keys must be strings")
    value_type = type(value)
    assert(value_type == "string"
        or value_type == "number"
        or value_type == "integer"
        or value_type == "boolean"
        or value_type == "nil",
        "global values must be of type string, number, integer, boolean, or nil")
    _globals[key] = value
    _global_deltas[key] = value
end

-- This is called by the runtime before events are processed.
function _update_globals(new_values)
    _global_deltas = {}
    for k,v in pairs(new_values) do
        _globals[k] = v
    end
end

-- Program enable/disable constants.
-- Must be in sync with program.rs
PROGRAM_ENABLE_SIGNAL = 1
PROGRAM_DISABLE_SIGNAL = 2
PROGRAM_ENABLE_TOGGLE_SIGNAL = 3

-- Functions to enable/disable programs by name.
_program_enable_deltas = {}
function program_enable(program_name)
    assert(type(program_name) == "string", "program names must be strings")
    _program_enable_deltas[program_name] = PROGRAM_ENABLE_SIGNAL
end

function program_disable(program_name)
    assert(type(program_name) == "string", "program names must be strings")
    _program_enable_deltas[program_name] = PROGRAM_DISABLE_SIGNAL
end

function program_enable_toggle(program_name)
    assert(type(program_name) == "string", "program names must be strings")
    _program_enable_deltas[program_name] = PROGRAM_ENABLE_TOGGLE_SIGNAL
end

function _get_program_enable_deltas()
    deltas = _program_enable_deltas
    _program_enable_deltas = {}
    return deltas
end

-- clamp clamps x to [from, to]
function clamp(from, to, x)
    if x < from then
        return from
    elseif x > to then
        return to
    end
    return x
end

-- lerp interpolates linearly between from and to.
function lerp(from, to, x)
    return from + (to - from) * x
end

-- map_range maps x from one range to another.
function map_range(a_lower, a_upper, b_lower, b_upper, x)
    return b_lower + (x - a_lower) * (b_upper - b_lower) / (a_upper - a_lower)
end

-- map_to_value maps x to the 16-bit Submarine value range.
function map_to_value(from, to, x)
    return math.floor(map_range(from, to, LOW, HIGH, x))
end

function map_from_value(from, to, x)
    return map_range(LOW, HIGH, from, to, x)
end

function map_value_to_temperature(x)
    return map_from_value(-40,80,x)
end

function map_value_to_relative_humidity(x)
    return map_from_value(0,100,x)
end

function absolute_humidity(temperature, humidity)
    return (6.112*math.exp((17.67*temperature)/(temperature+243.5))*humidity*2.1674)/(273.15+temperature)
end

-- These are provided by the runtime during setup.
input_alias_address = {}
output_alias_address = {}
group_addresses = {}

function input_alias_to_address(alias)
    local addr = input_alias_address[alias]
    if addr == nil then
        error("unknown input alias: " .. alias)
    end
    return addr
end

function output_alias_to_address(alias)
    local addr = output_alias_address[alias]
    if addr == nil then
        error("unknown output alias: " .. alias)
    end
    return addr
end

function group_to_addresses(group)
    local addr = group_addresses[alias]
    if addr == nil then
        error("unknown group: " .. alias)
    end
    return addr
end

-- This is read by the runtime after each tick.
_output_values_by_address = {}

function set_alias(alias, value)
    _output_values_by_address[output_alias_to_address(alias)] = value
end

function set_group(group, value)
    for i, address in ipairs(group_to_addresses(group)) do
        _output_values_by_address[address] = value
    end
end

-- This is called by the runtime.
-- By calling tick() from within Lua and returning the table in just
-- one function call we avoid one trip through the C FFI.
function _tick(now)
    -- This clears the previous tick's map.
    -- It costs performance, but otherwise we cannot distinguish whether a program wrote a value
    -- during this tick or some previous tick.
    _output_values_by_address = {}
    tick(now)
    return _output_values_by_address
end

-- This is provided by the runtime before handling events (= before each tick), and before the setup function is called.
-- The copy provided for setup() contains the whole address space.
-- Subsequent copies provided for tick() only contain inputs the program is registered for.
input_values_by_address = {}

function get_alias(alias)
    local addr = input_alias_to_address(alias)
    local value = input_values_by_address[addr]
    if value == nil then
        error("invalid address: " .. addr)
    end
    return value
end

-- This will be set up by the runtime.
_event_handlers = {}

-- Event type constants, keep synchronized with Rust and the readme!
EVENT_TYPE_UPDATE = "update"
EVENT_TYPE_BUTTON_DOWN = "button_down"
EVENT_TYPE_BUTTON_UP = "button_up"
EVENT_TYPE_BUTTON_CLICKED = "button_clicked"
EVENT_TYPE_BUTTON_LONG_PRESS = "button_long_press"
EVENT_TYPE_ERROR = "error"

-- This is called by the runtime to handle events.
-- Do not modify, please.
-- Events are passed into Lua as one string. (Extensive research has shown this to be most performant...)
-- Within that string, events are separated by a ";".
-- Within one event, fields are separated by a space.
function _handle_events(events)
    for event in string.gmatch(events, "[^;]*") do
        --print(event)
        local i = 0
        local address = -1
        local typ = ""
        for field in string.gmatch(event, "%S*") do
            --print(field)
            if i == 0 then
                address = tonumber(field)
            elseif i == 1 then
                typ = field
                -- Depending on the type we might be able to call the handler already...
                if typ == EVENT_TYPE_BUTTON_DOWN or typ == EVENT_TYPE_BUTTON_UP then
                    _handle_no_arg_event(address, typ)
                    break
                end
            else
                -- A third part! That means we can definitely call the handler now!
                _handle_one_arg_event(address, typ, tonumber(field))
            end
            i = i + 1
        end
    end
end

function _handle_no_arg_event(address, typ)
    local handlers_for_address = _event_handlers[address]
    if handlers_for_address == nil then
        return
    end
    for _, h in pairs(handlers_for_address) do
        -- TODO match by more than just even type
        if h["type"] == typ then
            _G[h["handler"]](address, typ, -1)
        end
    end
end

function _handle_one_arg_event(address, typ, arg)
    local handlers_for_address = _event_handlers[address]
    if handlers_for_address == nil then
        return
    end
    for _, h in pairs(handlers_for_address) do
        -- TODO match by more than just even type
        if h["type"] == typ then
            _G[h["handler"]](address, typ, arg)
        end
    end
end
