# VHS demo for the json-view plugin: a syntax-colored, pretty-printed JSON view
# of the selected payload, toggled with `i`.
#   Render:  cargo build --release && vhs assets/json-view.tape
#
# Prerequisites: first connections.json entry points at a broker; the json-view
# plugin enabled in plugins/config.json; a retained JSON topic, e.g.:
#   mosquitto_pub -r -t sensors/outdoor -m '{"temp":12.5,"humidity":80,"battery":95}'
#   mosquitto_pub -r -t devices/thermostat -m '{"mode":"heat","target":22,"current":20.5}'

Output assets/json-view.gif

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

# Drill into a topic carrying a JSON payload.
Type "j"       Sleep 400ms   # home
Type "j"       Sleep 500ms   # sensors
Enter          Sleep 1s      # expand sensors
Type "j"       Sleep 1500ms  # sensors/outdoor  (raw JSON in the Payload pane)

# Toggle the plugin's structured view (selecting a topic starts on the raw view).
Type "i"       Sleep 2500ms  # -> pretty, syntax-colored JSON view
Type "2"       Sleep 2s      # focus the Payload pane to read it

# A second JSON topic (the view resets to raw per topic; toggle it again).
Type "1"       Sleep 500ms
Type "k"       Sleep 400ms   # sensors
Type "k"       Sleep 400ms   # home
Type "k"       Sleep 500ms   # devices
Enter          Sleep 900ms   # expand devices
Type "j"       Sleep 500ms   # devices/lamp
Type "j"       Sleep 1500ms  # devices/thermostat (raw JSON)
Type "i"       Sleep 2500ms  # -> pretty JSON view

Escape         Sleep 700ms
Type "q"       Sleep 1s
