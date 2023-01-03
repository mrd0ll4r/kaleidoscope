SOURCE_VERSION = 2

-- Constants
local mode_full_bright = 0
local mode_night = 1
local noise_speed = 0.1
local KETTE_1_OUTLET = 'outlet-2-s1'
local KETTE_2_OUTLET = 'outlet-2-s2'
local KETTE_3_OUTLET = 'outlet-4-s1'
local KETTE_4_OUTLET = 'outlet-8-s2'

-- Variables
local ketten_enabled = true
local light_mode = mode_full_bright -- this saves relay cycles

function setup()
    set_priority(3)
    add_output_alias(KETTE_1_OUTLET)
    add_output_alias(KETTE_2_OUTLET)
    add_output_alias(KETTE_3_OUTLET)
    add_output_alias(KETTE_4_OUTLET)

    add_event_subscription('button-glassdoor-right', EVENT_TYPE_BUTTON_DOWN, 'handle_down')
    add_event_subscription('button-glassdoor-right', EVENT_TYPE_BUTTON_LONG_PRESS, 'handle_long_press')
end

function handle_down(address, _typ)
    program_enable('lichterketten')
    ketten_enabled = not ketten_enabled
    if not ketten_enabled then
        light_mode = mode_night
    end
end

function handle_long_press(address, _typ, duration)
    light_mode = mode_full_bright
    ketten_enabled = true
end

function tick(now)
    local kette1_on = true
    local kette2_on = (math.floor(now/30) % 2 == 1)
    local kette3_on = (math.floor(now/30) % 2 == 0)
    local kette4_on = noise2d(4, now*noise_speed) > 0

    set_alias(KETTE_1_OUTLET,LOW)
    set_alias(KETTE_2_OUTLET,LOW)
    set_alias(KETTE_3_OUTLET,LOW)
    set_alias(KETTE_4_OUTLET,LOW)

    if ketten_enabled and get_global("global_enable") then
        if light_mode == mode_night then
            if kette1_on then
                set_alias(KETTE_1_OUTLET, HIGH)
            end
            if kette2_on then
                set_alias(KETTE_2_OUTLET, HIGH)
            end
            if kette3_on then
                set_alias(KETTE_3_OUTLET, HIGH)
            end
            if kette4_on then
                set_alias(KETTE_4_OUTLET, HIGH)
            end
         else
            set_alias(KETTE_1_OUTLET,HIGH)
            set_alias(KETTE_2_OUTLET,HIGH)
            set_alias(KETTE_3_OUTLET,HIGH)
            set_alias(KETTE_4_OUTLET,HIGH)
        end
    end
end
