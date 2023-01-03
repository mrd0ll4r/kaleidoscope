SOURCE_VERSION = 2

function setup()
    set_priority(1)
    set_slow_mode(true)

    add_output_alias('spoider-outer-r')
    add_output_alias('spoider-outer-g')
    add_output_alias('spoider-outer-b')
    add_output_alias('spoider-outer-w')
    add_output_alias('spoider-inner-up-r')
    add_output_alias('spoider-inner-up-g')
    add_output_alias('spoider-inner-up-b')
    add_output_alias('spoider-inner-up-w')
    add_output_alias('spoider-inner-down-r')
    add_output_alias('spoider-inner-down-g')
    add_output_alias('spoider-inner-down-b')
    add_output_alias('spoider-inner-down-w')
end

function tick(now)
    set_alias('spoider-outer-r', LOW)
    set_alias('spoider-outer-g', LOW)
    set_alias('spoider-outer-b', LOW)
    set_alias('spoider-outer-w', LOW)
    set_alias('spoider-inner-up-r', LOW)
    set_alias('spoider-inner-up-g', LOW)
    set_alias('spoider-inner-up-b', LOW)
    set_alias('spoider-inner-up-w', LOW)
    set_alias('spoider-inner-down-r', LOW)
    set_alias('spoider-inner-down-g', LOW)
    set_alias('spoider-inner-down-b', LOW)
    set_alias('spoider-inner-down-w', LOW)
end