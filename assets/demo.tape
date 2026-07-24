# VHS tape for the LazyMQTT README demo.
#   Install VHS:  brew install vhs   (needs ttyd + ffmpeg)
#   Render:       cargo build --release && vhs assets/demo.tape
#
# Expects a broker connection configured as the first entry in
# connections.json, and a broker with some retained topics to browse
# (e.g. publish a few:  mosquitto_pub -t home/livingroom/temperature -m 21.4 -r).

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
Enter          Sleep 1s      # expand the top topic node
Type "j"       Sleep 400ms
Type "j"       Sleep 400ms
Enter          Sleep 1s      # expand a child node
Type "j"       Sleep 500ms   # land on a leaf topic (Payload updates)
Sleep 1500ms
Type "2"       Sleep 2s      # focus the Payload pane
Type "3"       Sleep 800ms   # focus the History pane
Enter          Sleep 1800ms  # expand a message entry inline
Type "z"       Sleep 1500ms  # collapse the History pane
Type "z"       Sleep 1500ms  # expand it again
Type "1"       Sleep 800ms   # back to the Topics pane
Escape         Sleep 800ms   # disconnect
Type "q"       Sleep 1s      # quit
