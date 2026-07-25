# VHS demo for the topic-alerts plugin: per-connection rules that raise an
# annotation + status-bar alert on values, changes, or silence — including
# numbers pulled from a JSON field.
#   Render:  cargo build --release && vhs assets/topic-alerts.tape
#
# Prerequisites: first connections.json entry points at a broker; the
# topic-alerts plugin enabled; that connection's rules seeded at
# plugins/alerts/<connection-id>.json with rules that fire on the retained data,
# e.g. temperature "above 20" (warn) and sensors/outdoor field temp "below 15"
# (error), with retained topics:
#   mosquitto_pub -r -t home/livingroom/temperature -m 21.4
#   mosquitto_pub -r -t sensors/outdoor -m '{"temp":12.5,"humidity":80}'

Output assets/topic-alerts.gif

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
Enter          Sleep 2500ms  # connect — rules fire on retained messages (status bar)

# The error alert reads a number from a nested JSON field (outdoor.temp < 15).
Type "j"       Sleep 400ms   # home
Type "j"       Sleep 500ms   # sensors
Enter          Sleep 900ms   # expand sensors
Type "j"       Sleep 700ms   # sensors/outdoor
Type "2"       Sleep 3s      # Payload pane: the ERROR alert annotation

# Open the per-connection rules editor.
Type "1"       Sleep 600ms
Type "A"       Sleep 2s      # rules list for this connection
Type "j"       Sleep 1500ms  # highlight the second rule
Type "k"       Sleep 1500ms
Type "e"       Sleep 2500ms  # open a rule to show the structured form
Escape         Sleep 800ms   # back to the list
Escape         Sleep 800ms   # back to the broker

Escape         Sleep 700ms   # disconnect
Type "q"       Sleep 1s
