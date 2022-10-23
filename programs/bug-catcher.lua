SOURCE_VERSION = 2

function setup()
    set_priority(3)
    set_slow_mode(true)
    add_output_alias('outlet-8-s1')
end

function tick(now)
    set_alias('outlet-8-s1',LOW)

    if get_global("global_enable") then
        set_alias('outlet-8-s1', HIGH)
    end
end

