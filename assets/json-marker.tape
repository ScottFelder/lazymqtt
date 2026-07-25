# VHS demo for the json-marker plugin: annotates each message with whether its
# payload is valid JSON (a green "valid JSON" / dim "not JSON" note).
#   Render:  cargo build --release && vhs assets/json-marker.tape
#
# Prerequisites: first connections.json entry points at a broker; the
# json-marker plugin enabled in plugins/config.json; a mix of retained topics,
# some JSON and some not, e.g.:
#   mosquitto_pub -r -t sensors/outdoor -m '{"temp":12.5,"humidity":80}'
#   mosquitto_pub -r -t devices/lamp -m on
#   mosquitto_pub -r -t status/uptime -m '3d 4h 12m'

Output assets/json-marker.gif

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

# A JSON payload -> "valid JSON" (green).
Type "j"       Sleep 400ms   # home
Type "j"       Sleep 500ms   # sensors
Enter          Sleep 900ms   # expand sensors
Type "j"       Sleep 800ms   # sensors/outdoor
Type "2"       Sleep 2500ms  # Payload pane: the json-marker note is shown

# A non-JSON payload -> "not JSON".
Type "1"       Sleep 500ms
Type "k"       Sleep 400ms   # sensors
Type "k"       Sleep 400ms   # home
Type "k"       Sleep 500ms   # devices
Enter          Sleep 900ms   # expand devices
Type "j"       Sleep 800ms   # devices/lamp  ("on")
Type "2"       Sleep 2800ms  # Payload pane: "not JSON"

Escape         Sleep 700ms
Type "q"       Sleep 1s
