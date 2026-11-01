# STCM2L opcodes

This file describes how the Switch release runs an STCM2L script, and what
each opcode does. The addresses are in the `main` executable of the JP
release 1.0.0, loaded at `0x7100000000` by the Ghidra Switch Loader.

Each meaning has one of these grades:

- **code**: the decompiled handler shows the meaning.
- **data**: the parameters in the 706 scripts show the meaning, for example
  an asset name or a file number. The handler is not read yet.
- **guess**: the parameters suggest a meaning. It is not verified.

## How to repeat the analysis

1. Install the Ghidra Switch Loader for Ghidra 12.1.
2. Import `exefs/main` of your dump into a project named `dlge`, and run the
   auto-analysis. It takes about 5 minutes.
3. Query the project with `tools/ghidra/query.sh`. For example,
   `query.sh dec:0x746c0` decompiles the run loop.

## The loader

`FUN_71000022f0` makes the name `<script>.dat`, reads the file from
`script.cpk`, and copies it into one buffer at `0x7100b5d600`. The buffer
holds 0x80000 bytes. A larger script stops the game with an assertion.

`FUN_7100002be0` loads a script and starts it at a label. The default label
is `sure1`.

## Start at a label

`FUN_7100074570(context, script, label)` prepares a run.

1. It checks the magic: the first 5 bytes are `STCM2` and byte 5 is `L`.
2. It finds the label in the export entries. Each entry is 40 bytes. It
   compares the name and takes the address.
3. It accepts the header value at `0x28` only if it is 1 or 2. This value
   is a format version.
4. It sets the program counter to `script + address`.

## The run loop

`FUN_71000746c0(vm, context, ...)` runs actions until a handler stops it.
For each action:

- If the call flag is not zero, the VM pushes a return frame and jumps to
  `script + opcode`. The call stack holds frames of 0x1b words.
- If the opcode is `0xffffff00` to `0xffffff09`, the VM runs a built-in
  operator of the first group.
- If the opcode is `0xffffff0a` to `0xffffff14`, the VM runs a built-in
  operator of the second group.
- The operators select an int, a float, or a 64-bit form from the types of
  their operands.
- If the opcode is below `0x1000`, the VM calls `vm[opcode]`, a handler from
  a table at the start of the VM object `0x7100b555d8`.

A handler returns a set of bits:

| Bit | Result |
|---|---|
| 0 | Go to the next action: `pc += length`. |
| 1 | Stop the loop for this frame. The script continues in a later frame. |
| 2 | End the script and clear the context. |

A handler that returns 0 sets the program counter itself, for example a
jump.

## Operands

`FUN_7100074010` resolves the params of an action before the handler
runs. The top 2 bits of `p0` select the mode:

| `p0` | Meaning |
|---|---|
| below `0x40000000` | A data block of the script, local or global. `p1` and `p2` are array indexes. No param of the 706 scripts uses an index. |
| `0x40000000` to `0x7fffffff` | An integer: the low 30 bits. No param of the scripts uses this mode. |
| `0x80000000` to `0xbfffffff` | A variable on a stack. No param of the scripts uses this mode. |
| `0xffffff00` to `0xffffff1f` | Argument `n` of the current call. The argument is a reference to the place that the caller gave. |
| `0xffffff20` to `0xffffff23` | Integer registers. The scripts use only `0xffffff20`. |
| `0xffffff24` to `0xffffff27` | Float registers. |
| `0xffffff28` and up | 64-bit registers. |
| `0xffffff40`, `0xffffff41`, `0xffffff42` | A data address, a code address, and a value at a code address. |

The listing shows the arguments and the registers as `#n`. The scripts use
the arguments 0 to 7 and `#0x20`.

## How a chapter runs

Each script is a chapter. The run loop runs a label, for example `sure1`,
to the return at the top level in one frame. Most handlers queue a
command for the scene engine with `FUN_71000198d0`. The flag commands are
also queued. A flag read is immediate. So a run reads the flags from
before its own commands.

The scene engine plays the commands. Then it runs a label again:

- After `0x44e`, it runs the label that `0x44e` names.
- After `0x44f`, it runs a label of another script.
- After `0x700` or `0x4bf`, the chapter is over.
- Else it runs the same label again.

A label reads the flag `switch` at its start and jumps to a part. At the
end of each part, the script adds 1 to `switch` and returns. So the next
run of the label shows the next part. A save holds the state between two
runs.

`dlplayer run` and the `vm` crate run all 706 chapters in this way. Each
chapter reaches its end in each run, with each choice.

## The handler table

`FUN_71000722c0` registers the opcodes from 0. `FUN_7100071490` calls four
functions that register the opcodes from 1000 (`0x3e8`):
`FUN_710006d4d0`, `FUN_710006f380`, `FUN_710006ff00`, and `FUN_7100070df0`.
Each one calls `FUN_7100074f60(vm, opcode, handler)`.

These functions register 764 opcodes. 369 have a handler. 395 have no
handler. The scripts use 76 opcodes. Opcodes `0x69b` and `0x69c` appear in
137 actions each, but these functions do not register them. Opcode `0x64f`
is registered with no handler.

Most handlers do not act directly. They make a command object and queue it
for the scene engine with `FUN_71000198d0`. The game has no C++ type
information, so the classes of the command objects have no names.

## Built-in operators

The run loop does these operators itself. The code is at `0x7100074990`.

| Index | First group: `dest = a op b` | Second group: `dest op= value` |
|---|---|---|
| 0 | `0xffffff00` add | `0xffffff0a` add |
| 1 | `0xffffff01` subtract | `0xffffff0b` subtract |
| 2 | `0xffffff02` multiply | `0xffffff0c` multiply |
| 3 | `0xffffff03` divide | `0xffffff0d` divide |
| 4 | `0xffffff04` remainder | `0xffffff0e` remainder |
| 5 | `0xffffff05` shift left | `0xffffff0f` shift left |
| 6 | `0xffffff06` arithmetic shift right | `0xffffff10` arithmetic shift right |
| 7 | `0xffffff07` and | `0xffffff11` and |
| 8 | `0xffffff08` xor | `0xffffff12` xor |
| 9 | `0xffffff09` or | `0xffffff13` or |
| 10 | | `0xffffff14` assign |

- The first group takes 3 params: `dest`, `a`, and `b`. The second group
  takes 2 params: `dest` and `value`.
- A division or a remainder by zero gives zero.
- The VM selects an int, a float, or a 64-bit form from the types of the
  operands. The type codes are 0 for int, 1 for float, and 7 for the
  64-bit form. In the first group, the float form has only add, subtract,
  multiply, and divide.

## Comparisons

Opcode `0x003` calls a comparison function from a table at
`0x7100aebc60`. Param 1 is the index. Each function compares param 0 with
param 2:

| Index | Comparison |
|---|---|
| 0 | `>` |
| 1 | `<` |
| 2 | `>=` |
| 3 | `<=` |
| 4 | `==` |
| 5 | `!=` |

The scripts use only 4 and 5.

## Control flow

| Opcode | Count | Grade | Meaning | Handler |
|---|---|---|---|---|
| call | 83115 | code | Push a return frame and jump to the target. | run loop |
| `0x000` | 45395 | code | Return. Pop a frame and go to the action after the call. With no frame, end the script. | `71000714e0` |
| `0x003` | 88641 | code | Compare param 0 and param 2 with the operator in param 1. If the result is false, jump to param 3. See [Comparisons](#comparisons). | `7100073f20` |
| `0x006` | 25160 | code | Jump to param 0. | `71000715d0` |
| `0xffffff14` | 99310 | code | Assignment: `param 0 = param 1`. See [Built-in operators](#built-in-operators). | run loop |
| `0xffffff02` | 2552 | code | Multiplication: `param 0 = param 1 * param 2`. | run loop |
| `0x44e` | 3731 | code | Queue a jump to the label in param 0, for example `sure2`. The jump happens after the run. | `710006be30` |
| `0x44f` | 833 | code | Queue a jump to a script and a label. Param 0 is the number of the script in `script.bin`, in the same game. Param 1 is the label. The prologues and `start` use it. | `710006be50` |
| `0x700` | 1361 | data | End the chapter. The handler queues a command. Each chapter ends with it, except the endings. | `7100008c10` |
| `0x4bf` | 754 | data | End the scene. The endings and the replay of the gallery (`memory_exit`) end with it. | `710006ca60` |

## Text and input

| Opcode | Count | Grade | Meaning | Handler |
|---|---|---|---|---|
| `0x4ba` | 134503 | code | Add a line of text to the message. Param 0 is the text. The handler adds a line break before each line after the first. | `710006c9b0` |
| `0x4bb` | 110345 | code | Show the message. If the message has text, the handler queues a message command with all the lines and clears the buffer. | `710006c9d0` |
| `0x4bc` | 75921 | code | Queue a command with the integer in param 0. Values 0 to 267, mostly 0. Probably the speaker. | `710006c9f0` |
| `0x4be` | 16657 | code | Queue a command with the integer in param 0. Values -120 to 600, mostly 60, 100, 15, and 30. Probably a wait. | `710006ca30` |
| `0x4dd` | 7958 | guess | Param 0 is 0 or 1. Probably a switch for a text mode. | `710006cbc0` |

## Audio

| Opcode | Count | Grade | Meaning | Handler |
|---|---|---|---|---|
| `0x462` | 37562 | data | Play a voice. Param 0 is the voice number. `140005` is `mb_140005.hca` or `ve_140005.hca` in `voice.cpk`. | `710006bff0` |
| `0x464` | 11582 | data | Play a sound effect. Param 0 is the number. `325` is `325.hca` in `se.cpk`. | `710006c050` |
| `0x465` | 8688 | guess | No params. Probably stop the sound effect. | `710006c0a0` |
| `0x690`, `0x691`, `0x695`, `0x696` | 5574 | guess | Two params: a number and a time in milliseconds such as 250, 1500, or 3000. Probably music and loop sounds with fades. | `710006fce0`, `710006fd30`, `710006fda0`, `710006fdf0` |

## Pictures

| Opcode | Count | Grade | Meaning | Handler |
|---|---|---|---|---|
| `0x3fa` | 5740 | data | Param 0 is a background name such as `bg_black`. Probably show a background with a transition. | `710006b710` |
| `0x3fb` | 4138 | guess | Param 0 is mostly `#0` or `#1`. Two actions name a picture such as `cg2031`. | `710006b7a0` |
| `0x402` | 1305 | data | Param 0 is a foreground name such as `fg_kako`. | `710006b830` |
| `0x403` | 1737 | guess | Probably clear the foreground. | `710006b8c0` |
| `0x502` | 2 | data | Param 0 is a character sprite name such as `ayato_01`. | `710006cf00` |
| `0x506` | 2826 | guess | Param 0 is mostly `#0`. Two actions name a character sprite such as `ayato_03U`. | `710006d030` |
| `0x504` | 2824 | guess | 9 params. Probably the position and look of a character sprite. | `710006cf20` |
| `0x482` | 1415 | guess | Param 0 is mostly `#0`. Some actions name a character such as `laito_03U`. | `710006c490` |
| `0x488`, `0x489`, `0x48a` | 16296 | guess | Values such as 255, 128, and 30. Probably a fade or a colour effect with a time. | `710006c5b0`, `710006c660`, `710006c6e0` |
| `0x3f2` | 706 | guess | Values 960 and 544. Probably the screen setup at the start of each script. | `710006b670` |
| `0x3f4`, `0x4f4` | 2280 | guess | Offsets such as -150 and -200 and a scale of 1.2. Probably a camera move or a shake. | `710006b690`, `710006cd20` |
| `0x510`, `0x513` | 16944 | guess | Small integers. Probably screen effects. | `710006d140`, `710006d240` |

## Movies

| Opcode | Count | Grade | Meaning | Handler |
|---|---|---|---|---|
| `0x44b` | 728 | data | Param 0 is a movie name such as `MOVIE1.usm` or `ve_ed`. | `710006bdf0` |

## Flags and choices

| Opcode | Count | Grade | Meaning | Handler |
|---|---|---|---|---|
| `0x41b` | 7010 | code | Queue: set the script flag in param 0 to the value in param 1. The script flags are in `flag.bin`. | `710006ba90` |
| `0x41c` | 1380 | code | Queue: add the value in param 1 to the script flag in param 0. The scripts use it only as `switch`, 1. | `710006bae0` |
| `0x41d` | 4423 | code | Read the script flag in param 0 into the variable in param 1, at once. | `710006bb30` |
| `0x425` | 1409 | code | Queue: set the system flag in param 0 to the value in param 1. The system flags are in `flagsys.bin`, for example `rei_Maniac1_end` at the end of a chapter. | `710006bbb0` |
| `0x427` | 15359 | code | Read the system flag in param 0 into the variable in param 1, at once. | `710006bc50` |
| `0x599` | 500 | code | Queue a choice. Param 0 is `select`. Param 1 is the name of the choice in `select.bin`. The pick goes to the script flag `select`: 0 for the first option. | `710006ee90` |
| `0x6fe` | 4 | data | Show the character select. The next part reads the script flag `character` as 1 to 6. `ve_prologue_00` and `mb_prologue_01` use it. | `710000b1c0` |

## Opcodes that do nothing

The handlers of these opcodes only return 1. On the Switch, they do
nothing: `0x467`, `0x46a`, `0x492`, `0x493`, `0x494`, `0x495`, `0x61a`, and
`0x668`.

## Opcodes with no handler

`0x64f` is registered with no handler. `0x69b` and `0x69c` are not
registered. No other code writes their slots in the handler table.

For an empty slot, the run loop calls the assertion handler and does not
move the program counter. The same action then runs again. But the runs
of the `vm` crate reach these actions in 167 scripts, in the main story.
So code that we have not found must fill the slots at run time. The `vm`
crate gives these actions to the host and goes on.

`0x69a` comes before `0x69b`. In `mb_aya_Dark_09`, two `0x69a` actions with
the params `0, 514, 283, 38004` and `1, 388, 202, 38005` come before one
`0x69b`.

## Other opcodes

The handlers of these opcodes pass their params to an engine function.
The meaning is not known yet.

| Opcode | Count | Params that the handler reads | Engine function |
|---|---|---|---|
| `0x456` | 2685 | int, int | `FUN_710000a9f0(a, 0, b)` |
| `0x457` | 1043 | none | `FUN_710000aa90(1000, 0)` |
| `0x466` | 47 | int, int, int | `FUN_710000abb0` |
| `0x47e` | 706 | 3 strings, 4 ints | `FUN_7100009a60` |
| `0x47f` | 706 | 3 strings | `FUN_7100009b20` |
| `0x480` | 1412 | value, 4 ints | `FUN_7100009740` |
| `0x483` | 706 | value, 3 ints | `FUN_71000098e0` |
| `0x491` | 706 | 4 ints | `FUN_71000096a0` |
| `0x5a7` | 393 | none | `FUN_7100008f70(1)` |
| `0x5a8` | 392 | none | `FUN_7100008f70(0)` |
| `0x5a9` | 24 | none | `FUN_7100009000` |
| `0x5be` | 2 | none | `FUN_710000b0e0` |
| `0x5cd` | 978 | none | `FUN_710000a7a0` |
| `0x5ce` | 978 | none | `FUN_710000a810` |
| `0x61c` | 706 | 3 strings | `FUN_7100009d70` |
| `0x61d` | 706 | 3 strings | `FUN_7100009e20` |
| `0x69a` | 215 | 4 ints | `FUN_710000a620` |
| `0x6b2` | 192 | none | `FUN_710000a490` |
| `0x6c2` | 15 | int | `FUN_710000a530` |
| `0x6c3` | 15 | none | `FUN_710000a5b0` |

`0x5a7` and `0x5a8` call the same function with 1 and 0. They are a pair
that turns one thing on and off.

## The typed decoder

`stcm2l::decode::decode` turns each action into an `Instruction`. The
instructions with a verified meaning are calls, returns, jumps, the
comparison, the built-in operators, text lines, the message, voices, and
sound effects. The opcodes that do nothing become `NoOp`. The opcodes with
no handler become `Unregistered`. All other opcodes become `Engine` with
their arguments.

In the 706 JP scripts, all 866704 actions decode. 645574 of them get a
verified meaning, and 221130 become `Engine`. In the 706 EN scripts, all
871025 actions decode.
