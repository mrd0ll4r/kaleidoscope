-- Output value constants
LOW = 0
HIGH = 65535

-- Seconds from some arbitrary epoch, set individually for each program.
START = 123.45

-- Contains the time of day in seconds since midnight, set by the runtime.
-- This example value is 14:36:12.
TIME_OF_DAY = 14*60*60 + 36*60 + 12

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

-- map_to_value maps x from [from, to] to the 16-bit Submarine value range.
function map_to_value(from, to, x)
    return math.floor(map_range(from, to, LOW, HIGH, x))
end

-- map_from_value maps x from the 16-bit Submarine value range to [from, to].
function map_from_value(from, to, x)
    return map_range(LOW, HIGH, from, to, x)
end

-- map_value_to_temperature maps from the 16-bit Submarine value range to temperatures in [-40, 80] °C.
function map_value_to_temperature(x)
    return map_from_value(-40,80,x)
end

-- map_value_to_temperature maps from the 16-bit Submarine value range to relative humidity in [0, 100] percent.
function map_value_to_relative_humidity(x)
    return map_from_value(0,100,x)
end

-- absolute_humidity calculates the absolute humidity from a temperature and relative humidity.
function absolute_humidity(temperature, humidity)
    return (6.112*math.exp((17.67*temperature)/(temperature+243.5))*humidity*2.1674)/(273.15+temperature)
end

-- Perlin noise functions, returning values in [-1,1].
-- These are implemented in Rust and relatively fast, although the higher-dimensional ones are always slower than
-- lower-dimensional ones.
function noise2d(x, y) return 0.0 end
function noise3d(x, y, z) return 0.0 end
function noise3d(x, y, z, t) return 0.0 end


-- =============================================
-- Setup-related things

function set_slow_mode(to) end

PARAMETER_TYPE_DISCRETE = 'discrete'
PARAMETER_TYPE_CONTINUOUS = 'continuous'

function new_discrete_parameter(name)
    local p={}
    p['_type'] = PARAMETER_TYPE_DISCRETE
    p['_name'] = name
    p['_i'] = 0
    p['_levels'] = {}
    return p
end

function add_discrete_parameter_level(p, level_name, level_description)
    local l={}
    l['_name'] = level_name
    l['_desc'] = level_description
    i = p['_i']
    p['_i'] = i+1
    p['_levels'][i] = l
    return p
end

function declare_discrete_parameter(p)
    _declare_parameter_generic(p)
end

function declare_continuous_parameter(name, lower_limit_incl, upper_limit_incl, default_value)
    local p={}
    p['_type'] = PARAMETER_TYPE_CONTINUOUS
    p['_name'] = name
    p['_lower'] = lower_limit_incl
    p['_upper'] = upper_limit_incl
    p['_default'] = default_value

    _declare_parameter_generic(p)
end


-- =============================================
-- Runtime-related things

_parameter_values = {}
function get_parameter_value(parameter_name)
    local p = _parameter_values[parameter_name]
    if p == nil then
        error("unknown parameter: " .. parameter_name)
    end
    return p
end

-- Maps output aliases to their address. Provided by the runtime.
output_alias_address = {}

function output_alias_to_address(alias)
    local addr = output_alias_address[alias]
    if addr == nil then
        error("unknown output alias: " .. alias)
    end
    return addr
end

-- This is returned to the runtime after each tick.
_output_values_by_address = {}

-- Set an output (by alias) to a value.
-- There is no buffering: If an output is not set during a tick, it's value will not be cached an re-sent to Submarine.
function set_alias(alias, value)
    _output_values_by_address[output_alias_to_address(alias)] = value
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