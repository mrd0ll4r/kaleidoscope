SOURCE_VERSION = 1

-- r,g,b,w are used to get Perlin noise from parallel 1d lines in a 2d space
local r = 0
local g = 1
local b = 2
local w = 3
local speed = 0.1

function setup()
    set_priority(3)
    add_output_group('strip3')
end

function tick(now)
    set_alias('strip3-r', map_to_value(-1, 1, noise2d(r, now * speed)))
    set_alias('strip3-g', map_to_value(-1, 1, noise2d(g, now * speed)))
    set_alias('strip3-b', map_to_value(-1, 1, noise2d(b, now * speed)))
    set_alias('strip3-w', map_to_value(-1, 1, noise2d(w, now * speed)))
end
