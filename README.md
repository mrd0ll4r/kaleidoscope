# Kaleidoscope

An environment to run Lua programs on top of Submarine.

## Description

This is a program that connects to Submarine via TCP and executes Lua programs to control Virtual Devices attached to
the Submarine instance.

## Configuration

## The Lua Runtime

We use [rlua](https://crates.io/crates/rlua), which means that our programs are to be Lua 5.3.

On a high level, the `Runtime` manages a list of `Program`s in Lua, which are evaluated repeatedly at a regular
interval.
We call this interval a Tick.

The Runtime evaluates all programs in parallel and aggregates their outputs, with higher-priority programs shadowing
outputs of lower-priority programs for the same addresses.
Priorities are unsigned integers in `[0,20]`.

At the beginning of each Tick, before the programs begin execution, the Runtime sends events that occurred since the last Tick to the
program.

At the end of each tick, all outputs are aggregated into one request and sent to Submarine.
Submarine then processes that request mostly-atomically.

### General Structure of Programs

Every program has a number of inputs and outputs as well as event filters associated with it.
These are registered when the program is loaded and cannot be modified later.

The reason for this is as follows:

- We need to know event filters to set up event routing/buffering and whatnot, which all runs in parallel to program
    execution.
    Modifying these later could potentially introduce stop-the-world pauses which are probably not good for realtime
    applications.
- We need to know outputs in order to build a priority queue for program execution.
    Asssume we have a program _A_ with priority 5 and a program _B_ with priority 10.
    Program _A_ registered addresses 22 and 23 as outputs, and program _B_ registered 21, 22, 23, and 24.
    We can now execute program _B_ to compute the values to set for addresses 21 to 24.
    After that we can skip execution of program _A_ because we know that its outputs would be shadowed by program _B_
    anyways.
    _However_, we can only do this optimization because we declared a program's output addresses as invariant.
- We need to know inputs to save some memory and copying.
    This is by far not as strict and necessary, but might enable some other optimizations later.

Every program consists of three basic building blocks:

#### The `setup` Function

When the Runtime loads a program, it calls its `setup()` function to set up inputs and outputs, register for events, and
determine the priority of a program.
This function should not be used to write outputs.

In the context of `setup()`, a bunch of special functions can be called, which are not available later:

- `set_priority(u8 <= 20)` sets the program's priority.
- `set_slow_mode(bool)` marks whether this program should run in slow mode.
    The default is false (i.e., fast mode).
    See below for more information about slow mode and program execution.
- `add_input_alias(string)` adds an alias to the inputs.
    This resolves the alias to its address and adds the numerical address to the program's outputs.
    It is checked whether the alias exists.
- `add_input_address(address)` adds a numerical address to the inputs.
    It is checked whether the address exists as a virtual device.
    _I recommend not using this_, because addresses might change and are tedious to maintain.
    Use the alias variant instead.
- `add_output_alias(string)` adds an alias to the outputs.
    This resolves the alias to its numerical address and behaves like `add_input_alias` in general.
- `add_output_address(address)` analogous to `add_input_address`, again not recommended.
- `add_output_group(string)` adds a group of addresses to the outputs.
    This resolves the group to its members' addresses and adds those to the outputs.
- `add_event_subscription(alias: string, type_name: string, target: string)` adds an event subscription with a type
    filter and a callable target.
    The first parameter is the alias of an input device from which to receive events.
    The second parameter is a filter for the type of the events to receive.
    Currently, possible values are `change`, `button_down`, `button_up`, `button_clicked`, and `button_long_press`.
    The last parameter is the name of a function (as a string!) to be called to handle the events.
    The handler function must handle three parameters:
    - The address (`u16`) of the event.
    - The type (`string`) of the event.
    - A value (`number`) contained in the event, which only applies to events of type `change` (which contains the new
        value), `button_clicked` (which contains the duration for which the button was pressed, as `f64` seconds),
        and `button_long_press` (which contains the number of seconds for which the button was pressed, as `u64`).
        All other events do not contain a value and `-1` will be passed to the handler.

TODO callable programs, rename inputs/outputs.

#### Event Handlers

A program may have event handlers, which behave as described above.
Programs do not have to work with events -- in particular, they should __not__ use events to update an internal "view"
of the address space.
The Runtime maintains this view automatically and makes it available to programs through the `get_(address|alias)`
functions, which are also much faster than event handlers.

It is not possible to set output values from within event handlers (or rather,
changes are only applied when `tick` runs next).
All program variables, however, can be modified.
For programs in slow mode, handling an event marks them to be executed in the
current tick and resets the slow mode timer.

Handling events is __slow__ in comparison to reading inputs and modifying outputs through `tick`.
This is rooted in the complexity associated with moving events from Rust-space to Lua-space and calling in between the
two, some performance numbers (and implementation frustrations) can be found throughout the source code.

To give two concrete examples:

- You have a program that changes the color of your living room to red when the temperature outside is above 40 °C.
    You read the temperature outside with a DHT22 sensor, so you can get a new value _at most_ every two seconds, not
    faster.
    The rate of `change` events in this case is low.
    You should use events and slow mode.
- You have a program that mirrors all lighting from your living room's RGBW LED strips to your toilet's.
    You could either copy-paste your code for the living room and somehow ensure the same programs are always running
    for your toilet, or you could write a program that sets the toilet lights to whatever is currently set for the
    living room (with one Tick delay).
    You should _not_ use events for this, because the rate of events is probably high.
    Instead, just `get` the values on each tick and `set` them for the toilet.

#### The `tick` Function

At the heart of every program is the `tick(now: f64)` function.
It takes one parameter, the current time in `f64` seconds since an unspecified epoch.

The Runtime usually calls this function on each Tick, but might decide not to. (See optimization notes above).
Because of this, the `tick` function __must not have side effects that rely on it being called on every Tick__.
As an example: Do not increment a counter on each Tick and calculate outputs based on it -- use the provided timestamp
to calculate outputs.

The `tick` function can call other functions and do whatever Lua can do, but it should run as fast as possible.
The Runtime keeps track of both the global Tick duration and `tick` durations for each program, which might be useful
for debugging.

#### Slow mode

Usually programs are run at every tick.
Slow mode programs are run every 1000 ticks or on event arrival.
The reason for this is that some programs can probably deal with the added
latency, which frees some performance for the programs that need to execute
every tick.
Events are still injected as soon as they arrive.
If an event matches for a program in slow mode, the slow mode counter is reset
and the program is run in the current tick.

#### Overwriting Values, Order of Execution

For every output address a priority-sorted list of programs writing to this
output is maintained.
On each tick, the runtime tries to find a minimal subset of programs to execute
this tick in order to fill every output address with the value of the
highest-priority program for this output, using a greedy algorithm:
If, for any output, there is an unexecuted program that writes to this output
with a higher priority than the assigned value (or no value is assigned yet),
the program is run and its outputs are assigned.

This could cause problems with slow mode programs, which "disappear" from the
list of runnable functions for the 999 ticks in which they are not run.
During this time, lower-priority programs could change the value, which would
lead to visual glitches.
The alternative to this would be basing occupation of an output address not on
whether a program has written a value to it, but rather whether any enabled
program has marked the address as its output.
This causes other problems, for example with high-priority programs writing
to many addresses, like stroboscopes or other global effects.
These high-priority programs would then "hog" the output at all times.
We'll have to see if this is actually an issue in practice.


### Builtins

The Runtime provides a bunch of builtin functions and constants, of which some are written in Rust and some in Lua.
These are:

- `KALEIDOSCOPE_VERSION: int`, which denotes the version of the Runtime.
- `START: f64` and `NOW: f64` denote the program epoch and current timestamp, both as `f64` seconds.
- `noise2d(f64, f64) -> f64` computes 2D Perlin noise in `[-1,1]`.
    This is implemented in Rust and relatively fast.
- `noise3d(f64, f64, f64) -> f64` computes 3D Perlin noise in `[-1,1]`.
    This is implemented in Rust and slower than the 2D version.
- `noise4d(f64, f64, f64, f64) -> f64` computes 4D Perlin noise in `[-1,1]`.
    This is implemented in Rust and slower than the 3D version.

The following are implemented in Lua and can be found in [src/builtin.lua](src/runtime/builtin.lua):

- `now() -> f64` gets the time in seconds since the program epoch.
- `clamp(from: numer, to: number, x: number) -> number` clamps `x` to `[from, to]`. 
- `lerp(from: number, to: number, x: number) -> number` interpolates between `from` and `to`.
- `map_range(a_lower: number, a_upper: number, b_lower: number, b_upper: number, x: number) -> number` maps `x` from the
    first range to the second.
- `map_to_value(from: number, to: number, x: number) -> u16` maps `x` from `[from,to]` to the 16-bit Submarine value
    range.
- `alias_to_address(alias: string) -> u16` translates an alias to a numerical address, if it exists.
    Raises an error otherwise.
- `group_to_addresses(group: string) -> [u16]` translates a group name to a list of addresses.
    Raises an error if the group does not exist.
- `set_alias(alias: string, value: u16)` sets the output at `alias` to `value`.
    Make sure to call this with integers, probably breaks with non-integers...
- `set_group(group: string, value: u16)` sets the group `group` to `value`.
    Make sure to call this with integers, probably breaks with non-integers...
- `get_alias(alias) -> u16` gets the value of the device at `alias`.
    Note that this is the most-up-to-date value from before the Tick was started.
    Specifically, values `set_` by other programs are not visible during the current tick.
- `EVENT_TYPE_UPDATE`, `EVENT_TYPE_BUTTON_DOWN`, `EVENT_TYPE_BUTTON_UP`, `EVENT_TYPE_BUTTON_CLICKED`, 
    `EVENT_TYPE_ERROR`, and `EVENT_TYPE_BUTTON_LONG_PRESS` are string constants for the event types.

## Example Programs

The [programs/](programs) directory contains a few example programs.
Specifically, you can look at them to see the following:

- [programs/sine.lua](programs/sine.lua) computes four time-shifted narrow sines for one RGBW strip.
    It does not use any events and shows how to organize code in a useful manner.
- [programs/noise.lua](programs/noise.lua) generates Perlin noise for one RGBW strip.
    Note that we use 2D Perlin noise with a fixed value for `x` and a time-dependent `y`.
    In practice, we sample parallel lines from the 2D space.
- [programs/motion-button.lua](programs/motion-button.lua) reacts to events for buttons and motion sensors to smoothly
    switch lighting.
    There is a couple of things to note here:
    - We show two different ways to interact with events, either by routing them to different handler functions or
        by routing them through one handler function and switching by address.
    - We show Lua tables.
    - We show that reading values and resolving aliases to addresses is possible in `setup()`.

## Compilation & Running

See the [README of Submarine](../submarine/README.md), which explains setup and cross-compilation for Linux on a Raspberry Pi.

In general, while it is not required to run Kaleidoscope on the same machine as Submarine, we have observed that this benefits lighting performance because of lower latency variance.

