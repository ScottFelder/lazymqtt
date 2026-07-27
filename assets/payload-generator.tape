# VHS demo for the payload-generator plugin: publish synthetic payloads
# (counters, random values, timestamps — optionally wrapped in a JSON template)
# to exercise subscribers. One-shot or streaming, driven from the `m` menu.
#   Render:  cargo build --release && vhs assets/payload-generator.tape
#
# Prerequisites: first connections.json entry points at a broker; the
# payload-generator plugin enabled; generators seeded in plugins/generators.json
# (a "count" counter stream and a "temp" random stream with a JSON template).

Output assets/payload-generator.gif

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

# Each generator is a command in the m menu.
Type "m"       Sleep 1500ms
Down 14        Sleep 1200ms  # highlight "Stream count → bench/count"
Enter          Sleep 2200ms  # start streaming — bench/count fills in and climbs

# Start the second generator (random, wrapped in a JSON template).
Type "m"       Sleep 800ms
Down 15        Sleep 1200ms  # highlight "Stream temp → bench/temp"
Enter          Sleep 2000ms

# Watch the generated values land in the tree.
Enter          Sleep 700ms   # expand "bench"
Type "j"       Sleep 400ms   # bench/count
Type "2"       Sleep 1600ms  # Payload pane: the counter climbing
Type "1"       Sleep 300ms
Type "j"       Sleep 400ms   # bench/temp
Type "2"       Sleep 1800ms  # Payload pane: {"temp": …}

Escape         Sleep 700ms
Type "q"       Sleep 1s
