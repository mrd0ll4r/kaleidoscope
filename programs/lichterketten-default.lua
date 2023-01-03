SOURCE_VERSION = 2

-- Constants
local KETTE_1_OUTLET = 'outlet-2-s1'
local KETTE_2_OUTLET = 'outlet-2-s2'
local KETTE_3_OUTLET = 'outlet-4-s1'
local KETTE_4_OUTLET = 'outlet-8-s2'

function setup()
    set_priority(1)
    set_slow_mode(true)

    add_output_alias(KETTE_1_OUTLET)
    add_output_alias(KETTE_2_OUTLET)
    add_output_alias(KETTE_3_OUTLET)
    add_output_alias(KETTE_4_OUTLET)
end

function tick(now)
    set_alias(KETTE_1_OUTLET,LOW)
    set_alias(KETTE_2_OUTLET,LOW)
    set_alias(KETTE_3_OUTLET,LOW)
    set_alias(KETTE_4_OUTLET,LOW)
end
