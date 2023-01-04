SOURCE_VERSION = 2

-- Constants
local r = 0
local g = 1
local b = 2
local w = 3
local inner = 0
local outer = 1
local sine_speed = 0.07
local noise_speed = 0.3
local white_low = 0.4
local white_high = 0.6
local color_low = 0.4
local color_high = 1.0
local gate_sine_speed = 0.2

local MODE_BRIGHTNESS_NAME = "brightness"
local MODE_BRIGHTNESS_NIGHT = 0
local MODE_BRIGHTNESS_FULL_BRIGHT = 1

local MODE_RINGS_NAME = "rings"
local MODE_RINGS_OFF = 0
local MODE_RINGS_SPOTS = 1
local MODE_RINGS_SPOTS_INNER = 2
local MODE_RINGS_SPOTS_INNER_OUTER = 3
local MODE_RINGS_SPOTS_OUTER = 4

-- Variables
local ring_enabled = false
local inner_enabled = false
local spots_enabled = false
local current_brightness_mode = MODE_BRIGHTNESS_NIGHT

function setup()
    set_priority(3)
    --add_output_group('kitchen')
    add_output_alias('kitchen-innen-r')
    add_output_alias('kitchen-innen-g')
    add_output_alias('kitchen-innen-b')
    add_output_alias('kitchen-innen-w')
    add_output_alias('kitchen-out-r')
    add_output_alias('kitchen-out-g')
    add_output_alias('kitchen-out-b')
    add_output_alias('kitchen-out-w')
    add_output_alias('kitchen-spots')

    -- Declare parameters
    declare_discrete_parameter(MODE_BRIGHTNESS_NAME, "day/night brightness mode",
        {build_discrete_parameter_value("Night", MODE_BRIGHTNESS_NIGHT),
         build_discrete_parameter_value("Full Bright", MODE_BRIGHTNESS_FULL_BRIGHT)
        }, current_brightness_mode, "handle_brightness_mode_change")


    declare_discrete_parameter(MODE_RINGS_NAME, "which parts of the kitchen light to turn on",
        {build_discrete_parameter_value("Off", MODE_RINGS_OFF),
         build_discrete_parameter_value("Spots", MODE_RINGS_SPOTS),
         build_discrete_parameter_value("Spots+Inner", MODE_RINGS_SPOTS_INNER),
         build_discrete_parameter_value("Spots+Inner+Outer", MODE_RINGS_SPOTS_INNER_OUTER),
         build_discrete_parameter_value("Spots+Outer", MODE_RINGS_SPOTS_OUTER)
        }, MODE_RINGS_SPOTS_INNER_OUTER, "handle_ring_mode_change")

    -- Turn on the lights, update variables accordingly
    handle_ring_mode_change(MODE_RINGS_SPOTS_INNER_OUTER)
end

function handle_brightness_mode_change(to)
    current_brightness_mode=to
end

function handle_ring_mode_change(to)
    if to == MODE_RINGS_OFF then
        ring_enabled = false
        inner_enabled = false
        spots_enabled = false
    elseif to == MODE_RINGS_SPOTS then
        ring_enabled = false
        inner_enabled = false
        spots_enabled = true
    elseif to == MODE_RINGS_SPOTS_OUTER then
        ring_enabled = true
        inner_enabled = false
        spots_enabled = true
    elseif to == MODE_RINGS_SPOTS_INNER_OUTER then
        ring_enabled = true
        inner_enabled = true
        spots_enabled = true
    elseif to == MODE_RINGS_SPOTS_INNER then
        ring_enabled = false
        inner_enabled = true
        spots_enabled = true
    end
end

function white_sine(index, now)
    local t = (now - START) * sine_speed
    return map_range(-1, 1, white_low, white_high, math.sin(t + (math.pi / 4) * index))
end

function color_noise(index,side, now)
    return map_range(-1,1,color_low,color_high,noise3d(index, side, now*noise_speed))
end

function gate_sine(index, now)
    local t = (now - START) * gate_sine_speed
    return clamp(0,1, map_range(-1,1,0,1,math.sin(t + math.pi * index))^0.001)
end

function compute_white(side,now)
    if current_brightness_mode == MODE_BRIGHTNESS_FULL_BRIGHT then
        return HIGH
    end
    local v = map_to_value(0,1,white_sine(w,now)*gate_sine(side,now))
    --print("white:", v)
    return v
end

function compute_color(index,side,now)
    if current_brightness_mode == MODE_BRIGHTNESS_FULL_BRIGHT then
        if side == inner then
            return HIGH/2
        else
            return HIGH
        end
    end
    local v = map_to_value(0,1,color_noise(index,side,now)*gate_sine(side,now))
    --print("color:", v)
    return v
end

function tick(now)
    if inner_enabled then
        set_alias('kitchen-innen-r', compute_color(r,inner,now))
        set_alias('kitchen-innen-g', compute_color(g,inner,now))
        set_alias('kitchen-innen-b', compute_color(b,inner,now))
        set_alias('kitchen-innen-w', compute_white(inner,now))
    end

    if ring_enabled then
        set_alias('kitchen-out-r', compute_color(r,outer,now))
        set_alias('kitchen-out-g', compute_color(g,outer,now))
        set_alias('kitchen-out-b', compute_color(b,outer,now))
        set_alias('kitchen-out-w', compute_white(outer,now))
    end

    if spots_enabled then
        set_alias('kitchen-spots', HIGH)
    end
end