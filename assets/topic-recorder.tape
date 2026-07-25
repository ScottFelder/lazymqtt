# VHS demo for the topic-recorder plugin: record a connection's traffic and
# replay it back to the broker, with a picker to replay / rename / delete
# individual recordings. Driven from the `m` command menu and the `R` picker.
#   Render:  cargo build --release && vhs assets/topic-recorder.tape
#
# Prerequisites: first connections.json entry points at a broker; the
# topic-recorder plugin enabled; a couple of recordings seeded for that
# connection under plugins/recordings/<connection-id>-<label>.jsonl.

Output assets/topic-recorder.gif

Set FontSize 16
Set Width 1200
Set Height 720
Set Padding 20
Set Theme "Catppuccin Mocha"

Hide
Type "./target/release/lazymqtt" Enter
Sleep 2s
Show

Sleep 1200ms
Enter          Sleep 1800ms  # connect

# The recorder's commands live in the command menu (record / replay / loop /
# prefix / speed — the option rows cycle with the arrow keys).
Type "m"       Sleep 2500ms
Escape         Sleep 800ms

# Open the recordings picker.
Type "R"       Sleep 2s      # lists this connection's recordings (newest first)

# Rename the highlighted recording in place.
Type "r"       Sleep 800ms
Backspace 11   Sleep 500ms
Type "front-door"  Sleep 900ms
Enter          Sleep 1800ms

# Pick another recording and replay it.
Type "j"       Sleep 1s      # move to "sensor-sweep"
Enter          Sleep 2500ms  # replay it -> republished under replay/…

# The replayed messages show up in the tree under the replay/ prefix.
Type "j"       Sleep 400ms   # home
Type "j"       Sleep 600ms   # replay
Enter          Sleep 900ms   # expand replay/
Type "j"       Sleep 500ms   # replay/lab
Enter          Sleep 900ms   # expand
Type "j"       Sleep 1800ms  # replay/lab/sensor

Escape         Sleep 700ms
Type "q"       Sleep 1s
