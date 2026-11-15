"""A window of text that scrolls by itself, for the call lab's display (lab.sh's
LAB_MOVING=desktop): a line every 1/15 s, back to the top at the end. Tk scrolls it by
moving what is drawn already on the display server and drawing only the new line, so it
takes next to no processor.

Usage: scroll.py <text file> <width>x<height>+<x>+<y>
"""

import sys
import tkinter

text = open(sys.argv[1], encoding="utf-8").read()
root = tkinter.Tk()
root.title("fogata-lab-moving")
root.geometry(sys.argv[2])
box = tkinter.Text(root, font=("DejaVu Sans Mono", 11), background="white",
                   foreground="#202020", wrap="word", borderwidth=0, highlightthickness=0)
box.insert("1.0", text)
box.pack(fill="both", expand=True)


def step():
    if box.yview()[1] >= 1.0:
        box.yview_moveto(0)
    else:
        box.yview_scroll(1, "units")
    root.after(66, step)


step()
root.mainloop()
