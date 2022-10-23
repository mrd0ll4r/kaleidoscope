-- This  is required, in case we ever make changes to the interpreter.
SOURCE_VERSION = 2

-- Constants
-- Times, in seconds since midnight
local MORNING_RUN_FROM = 10*60*60
local MORNING_RUN_TO = 11*60*60
local EVENING_RUN_FROM = 22*60*60
local EVENING_RUN_TO= 23*60*60

function setup()
    set_priority(3)
    set_slow_mode(true)
    add_output_alias('circulation-fan')
end

function tick(now)
    set_alias('circulation-fan', LOW)

    if TIME_OF_DAY > MORNING_RUN_FROM and TIME_OF_DAY < MORNING_RUN_TO then
        set_alias('circulation-fan', HIGH)
    elseif TIME_OF_DAY > EVENING_RUN_FROM and TIME_OF_DAY < EVENING_RUN_TO then
        set_alias('circulation-fan',HIGH)
    end
end
