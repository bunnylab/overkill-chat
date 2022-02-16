all: overkill-chat overkill-gui

overkill-gui: gui/src/overkill-gui.c
	(cd gui; gcc -o overkill-gui src/overkill-gui.c -Wall -Wextra -pedantic `pkg-config --cflags --libs gtk+-3.0` -rdynamic; mv overkill-gui ../overkill-gui )

overkill-chat: chat/src/main.rs
	(cd chat; cargo build --release; mv target/release/overkill-chat ../overkill-chat)

clean:
	rm overkill-gui overkill-chat

install:
	chmod +x install.sh
	./install.sh

uninstall:
	chmod +x uninstall.sh
	./uninstall.sh

