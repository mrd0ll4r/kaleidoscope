SOURCE_VERSION = 2

function setup()
    set_priority(1)
    set_slow_mode(true)
    add_output_alias('outlet-8-s1')
end

function tick(now)
    set_alias('outlet-8-s1',LOW)
end

