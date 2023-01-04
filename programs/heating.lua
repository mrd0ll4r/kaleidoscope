SOURCE_VERSION = 2

-- Mode Constants
local MODE_IN_NAME = "air-in-heating"
local MODE_IN_OFF = 0
local MODE_IN_HEAT_0 = 1
local MODE_IN_HEAT_1 = 2
local MODE_IN_HEAT_2 = 3
local MODE_IN_HEAT_3 = 4
-- Special mode to cool down after heating.
-- Excluded in the normal counting, will wrap automatically after some time.
local MODE_IN_COOLDOWN = 5

local MODE_OUT_NAME = "air-out"
local MODE_OUT_OFF = 0
local MODE_OUT_LOW = 1
local MODE_OUT_HIGH = 2

--- Constants for heater/air-in fan levels
local HEAT_0=0
local HEAT_1=10
local HEAT_2=20
local HEAT_3=30
local HEAT_0_FAN=6
local HEAT_1_FAN=1
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
local cooldown_until = NOW

function setup()
    set_priority(5)
    set_slow_mode(true)

    add_output_alias(ALIAS_FAN_IN)
    add_output_alias(ALIAS_FAN_OUT)
    add_output_alias(ALIAS_HEATER)

    declare_discrete_parameter(MODE_IN_NAME, "air in and heating",
        {build_discrete_parameter_value("Off", MODE_IN_OFF),
         build_discrete_parameter_value("Ventilation", MODE_IN_HEAT_0),
         build_discrete_parameter_value("Low heating", MODE_IN_HEAT_1),
         build_discrete_parameter_value("Medium heating", MODE_IN_HEAT_2),
         build_discrete_parameter_value("Maximum heating", MODE_IN_HEAT_3)
        }, current_mode_in, "handle_mode_in_change")

    declare_discrete_parameter(MODE_OUT_NAME, "air out",
        {build_discrete_parameter_value("Off", MODE_OUT_OFF),
         build_discrete_parameter_value("Low ventilation", MODE_OUT_LOW),
         build_discrete_parameter_value("High ventilation", MODE_OUT_HIGH)
        }, current_mode_out, "handle_mode_out_change")
end

function handle_mode_in_change(to)
    if is_heater_on() and to == MODE_IN_OFF then
        -- Go to cooldown mode
        cooldown_mode()
        return
    end

    -- Otherwise just apply the mode change
    current_mode_in = to
end

function handle_mode_out_change(to)
    current_mode_out = to
end

function cooldown_mode()
    current_mode_in = MODE_IN_COOLDOWN
    cooldown_until = NOW + COOLDOWN_SECONDS
end

function is_heater_on()
    return current_mode_in == MODE_IN_HEAT_1
        or current_mode_in == MODE_IN_HEAT_2
        or current_mode_in == MODE_IN_HEAT_3
end

function tick(now)
    set_alias(ALIAS_FAN_IN, LOW)
    set_alias(ALIAS_FAN_OUT, LOW)
    set_alias(ALIAS_HEATER, LOW)

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
    elseif current_mode_in == MODE_IN_COOLDOWN then
        if now > cooldown_until then
            -- Cooldown complete, change mode
            current_mode_in = MODE_IN_OFF
        else
            set_alias(ALIAS_FAN_IN, COOLDOWN_FAN)
            set_alias(ALIAS_HEATER, COOLDOWN_HEAT)
        end
    end

    if current_mode_out == MODE_OUT_LOW then
        set_alias(ALIAS_FAN_OUT, OUT_LOW_FAN)
    elseif current_mode_out == MODE_OUT_HIGH then
        set_alias(ALIAS_FAN_OUT, OUT_HIGH_FAN)
    end

end
