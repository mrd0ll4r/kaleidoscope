SOURCE_VERSION = 2

-- Constants
local MODE_OFF = 0
local MODE_ON = 1
-- Duration to stay on for, in seconds.
local MOTION_DURATION = 30*60
local FRONT_DOOR_LIGHT_ALIAS = 'light-outside-front-door'

-- Variables
local current_mode = MODE_OFF
local on_until = NOW

function setup()
    set_priority(5)
    set_slow_mode(true)

    add_output_alias(FRONT_DOOR_LIGHT_ALIAS)
    add_event_subscription('button-front-door-right', EVENT_TYPE_BUTTON_CLICKED, 'handle_click')
end

function handle_click(_addr, _typ, duration)
    if duration < 1.0 then -- seconds, float
        current_mode = (current_mode + 1) % 2
        if current_mode == MODE_ON then
            -- turn on for one period
            on_until = NOW + MOTION_DURATION
        end
    end
end

function tick(now)
    set_alias(FRONT_DOOR_LIGHT_ALIAS, LOW)

    if on_until < now then
        current_mode = MODE_OFF
    end

    if current_mode == MODE_OFF then
        return
    end
    if current_mode == MODE_ON then
        set_alias(FRONT_DOOR_LIGHT_ALIAS, HIGH)
        return
    end
end