LOW = 0
HIGH = 65535

-- This is set for each execution and represents a point in time when the program was started, as seconds from some point in time.
START = 123.45
-- This is set before handling events.
NOW = 123.45

-- now returns the time in seconds since the program epoch.
function now()
    return NOW
end

-- These are provided by the runtime during setup
function set_priority(p) end
function add_input_alias(alias) end
function add_output_alias(alias) end
function add_output_group(group) end
function add_event_subscription(alias, event_type, target_function_name) end
-- And these maybe during event handling too? (not yet)
function enable_tick() end
function disable_tick() end

-- Provided by the runtime:
-- Perlin noise functions, returning values in [-1,1]
function noise2d(x, y) return 0.0 end

function noise3d(x, y, z) return 0.0 end

function noise3d(x, y, z, t) return 0.0 end


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

-- These are provided by the runtime during setup.
alias_address = {}
group_addresses = {}

function alias_to_address(alias)
    local addr = alias_address[alias]
    if addr == nil then
        error("unknown alias: " .. alias)
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
    _output_values_by_address[alias_to_address(alias)] = value
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
    --_output_values_by_address = {} -- We could do this, but it costs some performance...
    tick(now)
    return _output_values_by_address
end

-- This is provided by the runtime before handling events (= before each tick), and before the setup function is called.
-- The copy provided for setup() contains the whole address space.
-- Subsequent copies provided for tick() only contain inputs the program is registered for.
input_values_by_address = {}

function get_alias(alias)
    local addr = alias_to_address(alias)
    local value = input_values_by_address[addr]
    if value == nil then
        error("invalid address: " .. addr)
    end
    return value
end

-- This will be set up by the runtime.
_event_handlers = {}

-- Event type constants, keep synchronized with Rust and the readme!
EVENT_TYPE_CHANGE = "change"
EVENT_TYPE_BUTTON_DOWN = "button_down"
EVENT_TYPE_BUTTON_UP = "button_up"
EVENT_TYPE_BUTTON_CLICKED = "button_clicked"
EVENT_TYPE_BUTTON_LONG_PRESS = "button_long_press"

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

--[[
function tablelength(T)
    local count = 0
    for _ in pairs(T) do count = count + 1 end
    return count
end

function dump(o)
    if type(o) == 'table' then
        local s = '{ '
        for k, v in pairs(o) do
            if type(k) ~= 'number' then k = '"' .. k .. '"' end
            s = s .. '[' .. k .. '] = ' .. dump(v) .. ','
        end
        return s .. '} '
    else
        return tostring(o)
    end
end

-- These are alternative implementation for the event handling to try out some things.
-- They did not perform...
function _handle_events_vec_thingy(events)
    for j, event in ipairs(events) do
        --print(event,dump(event))
        --print(event:has_value())
        if not event:has_value() then
            --print(event:address(), event:kind())
            _handle_no_arg_event(event:address(), event:kind())
        else
            --print(event:address(), event:kind(), event:value())
            _handle_one_arg_event(event:address(), event:kind(), event:value())
        end
    end
end

function _handle_events_vec(events)
    for j, event in ipairs(events) do
        --print(event,dump(event))
        if tablelength(event) == 2 then
            _handle_no_arg_event(tonumber(event[1]), event[2])
        else
            _handle_one_arg_event(tonumber(event[1]), event[2], tonumber(event[3]))
        end
    end
end
]]--