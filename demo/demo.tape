# VHS Hero Demo - Kanban Full Workflow
# Create card, edit details, switch views

Output demo.gif
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
Type "j"
Sleep 1000ms
Enter
Sleep 1000ms

# Beat 2: Create new card (4-12s)
Type "n"
Sleep 300ms
Type "Implement MCP server support"
Sleep 300ms
Enter
Sleep 1000ms
Enter
Sleep 1500ms

# Beat 3: Navigate to description and edit (18-26s)
Type "j"
Sleep 200ms
Type "j"
Sleep 200ms
Type "e"
Sleep 500ms
Type "i"
Sleep 300ms
Type "Add LLM integration via Model Context Protocol"
Sleep 500ms
Escape
Sleep 300ms
Type ":wq"
Sleep 800ms
Enter
Sleep 1500ms

# Beat 4: Close card details and toggle to list view. Show grouped-by-column list
Escape
Sleep 500ms
Type "V"
Sleep 1000ms
Type "k"
Sleep 200ms
Enter
Sleep 800ms

# Beat 5: Create a new sprint for unassigned cards
Escape
Sleep 800ms
Type "e"
Sleep 800ms
Type "j"
Sleep 200ms
Type "j"
Sleep 200ms
Type "j"
Sleep 200ms
Type "n"
Sleep 800ms
Type "features"
Sleep 1500ms
Enter
Sleep 500ms
Escape
Sleep 800ms
Enter
Sleep 800ms

# Beat 6: Assign cards to new sprint
Type "T"
Sleep 800ms
Space
Sleep 800ms
Escape
Sleep 800ms
Type "v"
Sleep 300ms
Type "j"
Sleep 300ms
Type "j"
Sleep 300ms
Type "v"
Sleep 100ms
Type "a"
Sleep 200ms
Type "j"
Sleep 1200ms
Space
Sleep 500ms
Enter
Sleep 300ms
Type "T"
Sleep 400ms
Space
Sleep 400ms
Type "j"
Sleep 200ms
Type "j"
Sleep 1100ms
Space
Sleep 800ms
Escape
Sleep 800ms
Shift+G
Sleep 1100ms
Shift+L
Sleep 900ms

# Beat 7: Copy git branch and quit app
Enter
Sleep 500ms
Shift+Y
Sleep 1000ms
Escape
Sleep 500ms
Escape
Sleep 500ms
Type "q"
Sleep 500ms
Paste
Sleep 800ms
Ctrl+C
Enter
Sleep 300ms
Type "# Implement your new feature"
Sleep 300ms
Enter
Sleep 300ms

# Beat 8: Get card by CLI
Type "kanban demo.json card get KAN-12 | jq"
Sleep 300ms
Enter
Sleep 1000ms

#Beat 9: Complete the card
Type "kanban demo.json card move KAN-12 --column Complete | jq"
Enter
Sleep 1300ms

# End
Sleep 3000ms

