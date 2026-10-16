# VHS Hero Demo - Kanban Full Workflow
# Create card, edit details, switch views

Output background.gif
Set FontSize 16
Set Width 1400
Set Height 800
Set Theme "Catppuccin Mocha"
Set Shell "bash"
Set Padding 10
Set Framerate 16
Set TypingSpeed 75ms

# Beat 1: Start app, select Kanban board (kanban view) (0-4s)
Type "kanban demo.json"
Enter
Sleep 1000ms

# Beat 2: Set background highlight
Type "S"
Sleep 250ms
Type "j"
Sleep 250ms
Type "j"
Sleep 250ms
Type "j"
Sleep 250ms
Type "j"
Sleep 250ms
Type "j"
Sleep 1000ms
Enter
Type "#000000"
Sleep 1000ms
Enter
Sleep 550ms

# Beat 3: Reset to default
Type "e"
Sleep 450ms
Type "j"
Sleep 750ms
Type "dd"
Sleep 550ms
Escape
Sleep 450ms
Type ":wq"
Sleep 350ms
Enter
Sleep 1050ms

# Beat 4: Set to #000080 with $EDITOR
Type "e"
Sleep 450ms
Type "j"
Sleep 450ms
Shift+A
Sleep 450ms
Escape
Sleep 450ms
Type "bb"
Sleep 450ms
Type "ciw"
Sleep 750ms
Type "000080"
Sleep 550ms
Escape
Sleep 750ms
Type ":wq"
Sleep 550ms
Enter
Sleep 750ms

# Beat 5: Go back to card list
Escape
Sleep 450ms
Enter
Sleep 550ms
Type "j"
Sleep 550ms
Type "j"
Sleep 550ms
Type "j"
Sleep 3500ms

