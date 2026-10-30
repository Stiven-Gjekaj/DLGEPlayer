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

## Control flow

| Opcode | Count | Grade | Meaning | Handler |
|---|---|---|---|---|
| call | 83115 | code | Push a return frame and jump to the target. | run loop |
| `0x000` | 45395 | code | Return. Pop a frame and go to the action after the call. With no frame, end the script. | `71000714e0` |
| `0x003` | 88641 | code | Compare param 0 and param 2. If the result is false, jump to param 3. Param 1 is 4 or 5, probably the operator. | `7100073f20` |
| `0x006` | 25160 | code | Jump to param 0. | `71000715d0` |
| `0xffffff14` | 99310 | code | A built-in operator of the second group. Param 0 is a global variable. Param 1 is a value. Probably an assignment. | run loop |
| `0xffffff02` | 2552 | code | A built-in operator of the first group, with 3 params. | run loop |
| `0x44e` | 3731 | data | Param 0 is a label such as `sure2`. Probably a jump to a label. | `710006be30` |
| `0x44f` | 833 | data | Param 1 is a label such as `sure3`. Probably a jump to a label in another script. | `710006be50` |

## Text and input

| Opcode | Count | Grade | Meaning | Handler |
|---|---|---|---|---|
| `0x4ba` | 134503 | code | Add a line of text to the message. Param 0 is the text. The handler adds a line break before each line after the first. | `710006c9b0` |
| `0x4bb` | 110345 | code | Show the message. If the message has text, the handler queues a message command with all the lines and clears the buffer. | `710006c9d0` |
| `0x4bc` | 75921 | code | Queue a command with the integer in param 0. Values 0 to 267, mostly 0. Probably the speaker. | `710006c9f0` |
| `0x4be` | 16657 | code | Queue a command with the integer in param 0. Values -120 to 600, mostly 60, 100, 15, and 30. Probably a wait. | `710006ca30` |
| `0x4bf` | 754 | guess | No params. | `710006ca60` |
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
| `0x41b` | 7010 | data | Param 0 is a flag name such as `switch` or `flg_memory`. Param 1 is a value. Probably set a flag. | `710006ba90` |
| `0x41c` | 1380 | data | Param 0 is a flag name. Probably clear or read a flag. | `710006bae0` |
| `0x41d` | 4423 | data | Param 0 is a flag name. Param 1 is a global variable. Probably read a flag into the variable. | `710006bb30` |
| `0x425` | 1409 | data | Param 0 is an ending name such as `rei_Maniac1_end`. Probably record an ending. | `710006bbb0` |
| `0x427` | 15359 | code | Look up the name in param 0 and write its number into the variable in param 1. The names are ending names such as `aya_ED1_end`. | `710006bc50` |
| `0x599` | 500 | data | Param 0 is `select`. Param 1 is a choice name. Probably a choice. | `710006ee90` |

## Other opcodes

These opcodes appear in the scripts. Their meaning is not known yet:
`0x456`, `0x457`, `0x466`, `0x467`, `0x46a`, `0x47e`, `0x47f`, `0x480`,
`0x483`, `0x491`, `0x492`, `0x493`, `0x494`, `0x495`, `0x5a7`, `0x5a8`,
`0x5a9`, `0x5be`, `0x5cd`, `0x5ce`, `0x61a`, `0x61c`, `0x61d`, `0x64f`,
`0x668`, `0x69a`, `0x69b`, `0x69c`, `0x6b2`, `0x6c2`, `0x6c3`, `0x6fe`, and
`0x700`.

Many of them appear exactly 706 times, one time in each script. They are
probably part of the start or the end of each script.
