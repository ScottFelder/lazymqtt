# VHS demo for the traffic-analytics plugin: a live stats dashboard in its own
# plugin-owned pane — message rate + peak with a sparkline, throughput, QoS mix,
# retained share, and a busiest-topics bar chart. Opened from the `m` menu.
#   Render:  cargo build --release && vhs assets/traffic-analytics.tape
#
# Prerequisites: first connections.json entry points at a broker; the
# traffic-analytics plugin enabled; live traffic flowing (the render script
# runs a background publisher feeding several topics).

Output assets/traffic-analytics.gif

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
Enter          Sleep 2500ms  # connect; analytics accumulates from the live stream

# Open the analytics pane from the command menu.
Type "m"       Sleep 1000ms
Down 14        Sleep 1200ms  # highlight "Traffic analytics"
Enter          Sleep 800ms   # open the pane

# It updates every frame — watch the rate, sparkline, and top-topics bars move.
Sleep 6s

Escape         Sleep 700ms
Type "q"       Sleep 1s
