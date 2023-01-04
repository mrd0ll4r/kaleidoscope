SOURCE_VERSION = 2

local outlets_floor_aliases = {'outlet-9-s', 'outlet-5-s', 'outlet-3-s', 'outlet-1-s', 'outlet-6-s'}
local outlets_ceiling_aliases = {'outlet-2-s1', 'outlet-2-s2', 'outlet-4-s1', 'outlet-4-s2', 'outlet-8-s2', 'outlet-8-s1'}
local kitchen_light_aliases = {'kitchen-innen-r','kitchen-innen-g','kitchen-innen-b','kitchen-innen-w','kitchen-out-r','kitchen-out-g','kitchen-out-b','kitchen-out-w','kitchen-spots'}
local toilet_light_aliases = {'klo-r','klo-g','klo-b','klo-w'}
local spoider_aliases = {'spoider-outer-r', 'spoider-outer-g', 'spoider-outer-b', 'spoider-outer-w', 'spoider-inner-up-r', 'spoider-inner-up-g', 'spoider-inner-up-b', 'spoider-inner-up-w', 'spoider-inner-down-r', 'spoider-inner-down-g', 'spoider-inner-down-b', 'spoider-inner-down-w'}

local FRONT_DOOR_LIGHT_ALIAS = 'light-outside-front-door'

local BLACKLIGHT_ALIAS = 'uv-hauptraum'
local RED_GREEN_LIGHT_ALIAS = 'red-green-party-light'
local party_light_aliases = {BLACKLIGHT_ALIAS, RED_GREEN_LIGHT_ALIAS}

local PUTZLICHT_FRONT_ALIAS = 'putzlicht-front'
local PUTZLICHT_ANBAU_ALIAS = 'putzlicht-anbau'
local putzlicht_aliases = {PUTZLICHT_FRONT_ALIAS, PUTZLICHT_ANBAU_ALIAS}

local all_aliases = {outlets_floor_aliases, outlets_ceiling_aliases, kitchen_light_aliases, toilet_light_aliases, spoider_aliases, {FRONT_DOOR_LIGHT_ALIAS}, party_light_aliases, putzlicht_aliases}

function setup()
    set_priority(1)

    for _, group in ipairs(all_aliases) do
        for _, alias in ipairs(group) do
            add_output_alias(alias)
        end
    end

    -- Turn off Putzlicht by default
    program_disable('putzlicht')
end

function tick(now)
    -- Turn on outlets near the floor
    for _, alias in ipairs(outlets_floor_aliases) do
        set_alias(alias, HIGH)
    end

    -- Turn off outlets on the ceiling
    for _, alias in ipairs(outlets_ceiling_aliases) do
        set_alias(alias, LOW)
    end

    -- Turn off kitchen light
    for _, alias in ipairs(kitchen_light_aliases) do
        set_alias(alias, LOW)
    end

    -- Turn off toilet lights
    for _, alias in ipairs(toilet_light_aliases) do
        set_alias(alias, LOW)
    end

    -- Turn off the spoider
    for _, alias in ipairs(spoider_aliases) do
        set_alias(alias, LOW)
    end

    -- Turn off Putzlicht
    for _, alias in ipairs(putzlicht_aliases) do
        set_alias(alias, LOW)
    end

    -- Turn off party lights
    for _, alias in ipairs(party_light_aliases) do
        set_alias(alias, LOW)
    end

    -- Turn off front door light
    set_alias(FRONT_DOOR_LIGHT_ALIAS, LOW)
end

