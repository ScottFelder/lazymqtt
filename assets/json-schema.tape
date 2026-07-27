# VHS demo for the json-schema plugin: validate messages against a
# per-connection JSON Schema mapped by topic, annotating valid (✓) / invalid
# (✗, with the failing path). Includes the in-app schema editor (`S`).
#   Render:  cargo build --release && vhs assets/json-schema.tape
#
# Prerequisites: first connections.json entry points at a broker; the
# json-schema plugin enabled; a schema for sensors/# in
# plugins/schemas/<connection-id>.json; retained sensors/* topics (one valid,
# one out-of-range, one missing a required field).

Output assets/json-schema.gif

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
Enter          Sleep 2s      # connect — schemas validate the retained messages

# Drill into the sensors topics and read the validation annotations.
Enter          Sleep 900ms   # expand "sensors"
Type "j"       Sleep 600ms   # sensors/bad  (missing required "temp")
Type "2"       Sleep 2500ms  # Payload pane: ✗ missing required property
Type "1"       Sleep 400ms
Type "j"       Sleep 600ms   # sensors/hot  (999 above maximum)
Type "2"       Sleep 2500ms  # Payload pane: ✗ /temp above maximum
Type "1"       Sleep 400ms
Type "j"       Sleep 600ms   # sensors/ok   (valid)
Type "2"       Sleep 2500ms  # Payload pane: ✓ valid

# The schemas are edited in-app.
Type "1"       Sleep 400ms
Type "S"       Sleep 1800ms  # schema list for this connection
Enter          Sleep 2500ms  # open the form: topic field + JSON body editor
Escape         Sleep 600ms
Escape         Sleep 700ms

Escape         Sleep 700ms   # disconnect
Type "q"       Sleep 1s
