SOURCE_VERSION = 2

-- Mode Constants
local MODE_IN_OFF = 0
local MODE_IN_HEAT_0 = 1
local MODE_IN_HEAT_1 = 2
local MODE_IN_HEAT_2 = 3
local MODE_IN_HEAT_3 = 4
-- Special mode to cool down after heating.
-- Excluded in the normal counting, will wrap automatically after some time.
local MODE_IN_COOLDOWN = 5
local NUM_MODE_IN = 5

local MODE_OUT_OFF = 0
local MODE_OUT_LOW = 1
local MODE_OUT_HIGH = 2
local NUM_MODE_OUT = 3

--- Constants for heater/air-in fan levels
local HEAT_0=0
local HEAT_1=10
local HEAT_2=20
local HEAT_3=30
local HEAT_0_FAN=6
local HEAT_1_FAN=2
local HEAT_2_FAN=2
local HEAT_3_FAN=2
local COOLDOWN_HEAT=0
local COOLDOWN_FAN=2
local COOLDOWN_SECONDS=180

--- Constants for air-out fan levels
local OUT_LOW_FAN=2
local OUT_HIGH_FAN=4

local ALIAS_FAN_IN='fan-heater-fan-in-level'
local ALIAS_FAN_OUT='fan-heater-fan-out-level'
local ALIAS_HEATER='fan-heater-heater-level'

-- Variables
local current_mode_in = MODE_IN_OFF
local current_mode_out = MODE_OUT_OFF
local cooldown_until = TIME_OF_DAY

function setup()
    set_priority(5)
    set_slow_mode(true)

    add_output_alias(ALIAS_FAN_IN)
    add_output_alias(ALIAS_FAN_OUT)
    add_output_alias(ALIAS_HEATER)
    add_event_subscription('button-bedroom-left', EVENT_TYPE_BUTTON_CLICKED, 'handle_air_out_click')
    add_event_subscription('button-bedroom-right', EVENT_TYPE_BUTTON_CLICKED, 'handle_air_in_click')
end

function cooldown_mode()
    current_mode_in = MODE_IN_COOLDOWN
    cooldown_until = TIME_OF_DAY + COOLDOWN_SECONDS
end

function is_heater_on()
    return current_mode_in == MODE_IN_HEAT_1
        or current_mode_in == MODE_IN_HEAT_2
        or current_mode_in == MODE_IN_HEAT_3
end

function handle_air_in_click(_addr, _typ, duration)
    if current_mode_in == (NUM_MODE_IN - 1) then
        cooldown_mode()
        return
    end
    current_mode_in = (current_mode_in + 1) % NUM_MODE_IN
end

function handle_air_out_click(_addr, _typ, duration)
    current_mode_out = (current_mode_out + 1) % NUM_MODE_OUT
end

function tick(now)
    set_alias(ALIAS_FAN_IN, LOW)
    set_alias(ALIAS_FAN_OUT, LOW)
    set_alias(ALIAS_HEATER, LOW)

    -- Cooldown mode runs regardless of global disable
    if current_mode_in == MODE_IN_COOLDOWN then
        if TIME_OF_DAY > cooldown_until then
            current_mode_in = MODE_IN_OFF
        else
            set_alias(ALIAS_FAN_IN, COOLDOWN_FAN)
            set_alias(ALIAS_HEATER, COOLDOWN_HEAT)
        end
    end

    local global_enable = get_global("global_enable")
    if not global_enable then
        -- If we had the heater on and then switched global_enable off, go into cooldown mode.
        if is_heater_on() then
            cooldown_mode()
        end
        return
    end

    if current_mode_in == MODE_IN_HEAT_0 then
        set_alias(ALIAS_FAN_IN, HEAT_0_FAN)
        set_alias(ALIAS_HEATER, HEAT_0)
    elseif current_mode_in == MODE_IN_HEAT_1 then
        set_alias(ALIAS_FAN_IN, HEAT_1_FAN)
        set_alias(ALIAS_HEATER, HEAT_1)
    elseif current_mode_in == MODE_IN_HEAT_2 then
        set_alias(ALIAS_FAN_IN, HEAT_2_FAN)
        set_alias(ALIAS_HEATER, HEAT_2)
    elseif current_mode_in == MODE_IN_HEAT_3 then
        set_alias(ALIAS_FAN_IN, HEAT_3_FAN)
        set_alias(ALIAS_HEATER, HEAT_3)
    end

    if current_mode_out == MODE_OUT_LOW then
        set_alias(ALIAS_FAN_OUT, OUT_LOW_FAN)
    elseif current_mode_out == MODE_OUT_HIGH then
        set_alias(ALIAS_FAN_OUT, OUT_HIGH_FAN)
    end

end
