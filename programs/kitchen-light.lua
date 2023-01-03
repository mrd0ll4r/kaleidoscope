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
local mode_full_bright = 0
local mode_night = 1

-- Variables
local ring_enabled = false
local inner_enabled = false
local spots_enabled = false
local ring_mode = 0
local light_mode = mode_night

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

    add_event_subscription('button-kitchen-left', EVENT_TYPE_BUTTON_DOWN, 'handle_press')
    add_event_subscription('button-kitchen-left', EVENT_TYPE_BUTTON_LONG_PRESS, 'handle_long_press')

    -- Turn on the lights, update variables accordingly
    update_rings(2)
end

function handle_press(_addr, _typ)
    program_enable('kitchen-light')
    update_rings(1)
end

function update_rings(change)
    ring_mode = (ring_mode + change + 5) % 5
    if ring_mode == 0 then
        ring_enabled = false
        inner_enabled = false
        spots_enabled = true
    elseif ring_mode == 1 then
        ring_enabled = true
        inner_enabled = false
        spots_enabled = true
    elseif ring_mode == 2 then
        ring_enabled = true
        inner_enabled = true
        spots_enabled = true
    elseif ring_mode == 3 then
        ring_enabled = false
        inner_enabled = true
        spots_enabled = true
    else
        ring_enabled = false
        inner_enabled = false
        spots_enabled = false
    end
end

function handle_long_press(address, _typ, duration)
    if light_mode == mode_night then
        light_mode = mode_full_bright
    else
        light_mode = mode_night
    end
    -- Fix up the rings, which were messed up by the short + long press
    --update_rings(-1)
end

function spot_sine(now)
    local t = (now - START) * spots_speed
    return map_to_value(0,1,map_range(-1,1,spots_low, spots_high, math.sin(t)))
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
    if light_mode == mode_full_bright then
        return HIGH
    end
    local v = map_to_value(0,1,white_sine(w,now)*gate_sine(side,now))
    --print("white:", v)
    return v
end

function compute_color(index,side,now)
    if light_mode == mode_full_bright then
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
    local global_enabled = get_global("global_enable")
    if inner_enabled and global_enabled then
        set_alias('kitchen-innen-w', compute_white(inner,now))

        set_alias('kitchen-innen-r', compute_color(r,inner,now))
        set_alias('kitchen-innen-g', compute_color(g,inner,now))
        set_alias('kitchen-innen-b', compute_color(b,inner,now))
    else
        set_alias('kitchen-innen-r', LOW)
        set_alias('kitchen-innen-g', LOW)
        set_alias('kitchen-innen-b', LOW)
        set_alias('kitchen-innen-w', LOW)
    end

    if ring_enabled and global_enabled then
        set_alias('kitchen-out-w', compute_white(outer,now))

        set_alias('kitchen-out-r', compute_color(r,outer,now))
        set_alias('kitchen-out-g', compute_color(g,outer,now))
        set_alias('kitchen-out-b', compute_color(b,outer,now))
    else
        set_alias('kitchen-out-r', LOW)
        set_alias('kitchen-out-g', LOW)
        set_alias('kitchen-out-b', LOW)
        set_alias('kitchen-out-w', LOW)
    end

    if spots_enabled and global_enabled then
        set_alias('kitchen-spots', HIGH)
    else
        set_alias('kitchen-spots', LOW)
    end
end