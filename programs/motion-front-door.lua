SOURCE_VERSION = 2

-- Constants
-- Run at night
--local begin_after = 20 * 60 * 60 -- seconds since midnight
--local end_before = 8 * 60 * 60
local MODE_OFF = 0
local MODE_MOTION = 1
local MODE_ON = 2
local MOTION_DURATION = 30 -- seconds
local FRONT_DOOR_LIGHT_ALIAS = 'light-outside-front-door'

-- Variables
local front_light_on = false
local current_mode = MODE_OFF
local on_until = 0

function setup()
    set_priority(5)
    set_slow_mode(true)

    add_input_alias('motion-sensor-front-door')
    add_output_alias(FRONT_DOOR_LIGHT_ALIAS)
    add_event_subscription('button-front-door-right', EVENT_TYPE_BUTTON_CLICKED, 'handle_click')

    -- The PIR is inverted, so it's HIGH when off. So by triggering on BUTTON_UP (= trigger down) we get its activation.
    add_event_subscription('motion-sensor-front-door', EVENT_TYPE_BUTTON_UP, 'handle_pir')
end

function handle_pir(_addr, _typ)
    if mode == MODE_MOTION then
        front_light_on = true
        on_until = TIME_OF_DAY + MOTION_DURATION
    end
end

function handle_click(_addr, _typ, duration)
    if duration < 1.0 then -- seconds, float
        current_mode = (current_mode + 1) % 3
        if current_mode == MODE_MOTION then
            -- turn on for one period
            front_light_on = true
            on_until = TIME_OF_DAY + MOTION_DURATION
        end
    end
end

function tick(now)
    set_alias(FRONT_DOOR_LIGHT_ALIAS, LOW)

    if current_mode == MODE_OFF then
        return
    end
    if current_mode == MODE_ON then
        set_alias(FRONT_DOOR_LIGHT_ALIAS, HIGH)
        return
    end

    if on_until < TIME_OF_DAY then
        front_light_on = false
    end

    if front_light_on then
--    if front_light_enabled and TIME_OF_DAY > begin_after and TIME_OF_DAY < end_before and not get_alias('motion-sensor-front-door') then
        set_alias(FRONT_DOOR_LIGHT_ALIAS, HIGH)
    end
end