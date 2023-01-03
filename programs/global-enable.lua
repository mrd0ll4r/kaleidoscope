SOURCE_VERSION = 2

-- Constants
local global_enable_long_press_seconds = 3
local default = true
local global_name = "global_enable"
local programs_to_turn_off = {'kitchen-light', 'bug-catcher', 'klo-light', 'lichterketten', 'spoider', 'party-light', 'putzlicht'}
local programs_to_turn_on = {'kitchen-light', 'bug-catcher', 'klo-light', 'lichterketten', 'spoider'}

-- Variables
local value = default
local button_front_door_address = input_alias_to_address('button-front-door-right')

function setup()
    set_priority(10)
    set_slow_mode(true)

    add_event_subscription('button-front-door-right', EVENT_TYPE_BUTTON_LONG_PRESS, 'handle_long_press')

    set_global(global_name, value)
end

function handle_long_press(address, _typ, duration)
    if address == button_front_door_address and duration == global_enable_long_press_seconds then
        value = not value
        if value then
            for _, p in ipairs(programs_to_turn_on) do
                program_enable(p)
            end
        else
            for _, p in ipairs(programs_to_turn_off) do
                program_disable(p)
            end
        end
--        set_global(global_name, value)
    end
end

function tick(now) end