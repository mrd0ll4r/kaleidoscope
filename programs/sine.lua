-- This is required, in case we ever make changes to the interpreter.
SOURCE_VERSION = 1

-- A bunch of "constants", which are not really constant, but whatever...
local r = 0
local g = 1
local b = 2
local w = 3
local speed = 0.1

-- setup is called once on program load
function setup()
    set_priority(3)
    add_output_alias('strip3-r')
    add_output_alias('strip3-g')
    add_output_alias('strip3-b')
    add_output_alias('strip3-w')
    -- or
    add_output_group('strip3')
end

-- you can use whatever helper functions you want
function compute_nice_sine(index, now)
    local t = (now - START) * speed
    -- you can use a lot of the Lua standard library, see the rlua documentation for more information.
    return map_to_value(0, 1, math.sin(t + (math.pi / 4) * index) ^ 12)
end

-- tick is called in a loop, with now=seconds since epoch (as float).
-- This must not have any side effects!
-- There is no guarantee tick will be called on each tick, e.g. if we know that
-- all outputs of this program will be shadowed by higher-priority outputs by other programs,
-- there is no need to evaluate this program for the current tick.
function tick(now)
    set_alias('strip3-r', compute_nice_sine(r, now))
    set_alias('strip3-g', compute_nice_sine(g, now))
    set_alias('strip3-b', compute_nice_sine(b, now))
    set_alias('strip3-w', compute_nice_sine(w, now))
end
