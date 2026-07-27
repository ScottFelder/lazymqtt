# VHS demo for the publish-templates plugin: saved topic/payload/QoS/retain
# presets that open the publish form pre-filled from the `m` menu; `^T` saves
# the current form as a new template.
#   Render:  cargo build --release && vhs assets/publish-templates.tape
#
# Prerequisites: first connections.json entry points at a broker; the
# publish-templates plugin enabled; a template seeded in
# plugins/publish-templates.json (e.g. "reboot").

Output assets/publish-templates.gif

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

# Recall a saved template from the command menu.
Type "m"       Sleep 800ms
Down 14        Sleep 1500ms  # highlight "Publish: reboot"
Enter          Sleep 2200ms  # opens the publish form pre-filled from the template
Enter          Sleep 1800ms  # send it — devices/gateway/cmd appears in the tree

# Save a new template from the publish form with ^T.
Type "p"       Sleep 900ms   # open publish
Tab            Sleep 400ms   # to the payload field
Type "ping"    Sleep 800ms
Ctrl+T         Sleep 900ms   # "save as template" prompt
Type "healthcheck"  Sleep 1000ms
Enter          Sleep 1800ms  # saved

Escape         Sleep 700ms
Type "q"       Sleep 1s
