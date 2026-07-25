# VHS tape for the LazyMQTT README demo — core functionality (no plugins).
#   Install VHS:  brew install vhs   (needs ttyd + ffmpeg)
#   Render:       cargo build --release && vhs assets/demo.tape
#
# Expects the first entry in connections.json to point at a broker, all plugins
# disabled (plugins/config.json), and a few retained topics to browse, e.g.:
#   mosquitto_pub -r -t home/livingroom/temperature -m 21.4
#   mosquitto_pub -r -t sensors/outdoor -m '{"temp":12.5,"humidity":80}'
#   mosquitto_pub -r -t devices/thermostat -m '{"mode":"heat","target":22}'

Output assets/demo.gif

Set FontSize 16
Set Width 1200
Set Height 720
Set Padding 20
Set Theme "Catppuccin Mocha"

# Launch the release binary (hide the command itself).
Hide
Type "./target/release/lazymqtt" Enter
Sleep 2s
Show

Sleep 1500ms
Enter          Sleep 2s      # connect to the first saved broker

# Browse the topic tree: expand branches and drill down to a leaf.
Type "j"       Sleep 500ms   # move to the "home" branch
Enter          Sleep 1s      # expand it
Type "j"       Sleep 400ms   # bedroom
Type "j"       Sleep 400ms   # kitchen
Type "j"       Sleep 500ms   # livingroom
Enter          Sleep 1s      # expand livingroom
Type "j"       Sleep 500ms   # humidity  (Payload updates)
Type "j"       Sleep 500ms   # light
Type "j"       Sleep 1500ms  # temperature

# Inspect the message: Payload and History panes.
Type "2"       Sleep 1800ms  # focus the Payload pane
Type "3"       Sleep 1s      # focus the History pane
Enter          Sleep 1800ms  # expand a message entry inline
Type "z"       Sleep 1200ms  # collapse the History pane (Payload fills the space)
Type "z"       Sleep 1200ms  # restore the split
Type "1"       Sleep 800ms   # back to the Topics pane

# Publish a message from the client. The form pre-fills the topic with the
# selected leaf, so just move to the payload field and type a value.
Type "p"       Sleep 1200ms  # open the publish form (topic pre-filled)
Tab            Sleep 600ms   # move to the payload field
Type "22.8"    Sleep 900ms
Enter          Sleep 1800ms  # send it (the tree updates)

Escape         Sleep 800ms   # disconnect
Type "q"       Sleep 1s      # quit
