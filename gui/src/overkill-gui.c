
#include <gtk/gtk.h>
#include <stdio.h>
#include <stdlib.h>
#include <unistd.h>
#include <fcntl.h>

#define GLADEFILEDEFAULT "gui/lib/starter.glade"
#define CSSFILEDEFAULT "gui/lib/starter.css"

struct widget_refs
{
    GtkTextView *log;  // message log
    GtkEntry *entry;  // entry box
    GtkScrolledWindow *scroll;  // scroll object
};

// global fd for subprocess stdin/stdout
struct widget_refs widgets;
char* css_file;
char* glade_file; 

static void scrolledWindowToBottomImp(GtkScrolledWindow *scrolledWin) {
    GtkAdjustment *adjustment = gtk_scrolled_window_get_vadjustment(scrolledWin);
    gtk_adjustment_set_value(adjustment, gtk_adjustment_get_upper(adjustment) - gtk_adjustment_get_page_size(adjustment));
    gtk_scrolled_window_set_vadjustment(scrolledWin, adjustment);
}


static void writeToChatWindow(const char *text, int n) {
    GtkTextBuffer* txtbuffer = gtk_text_view_get_buffer(widgets.log);
    GtkTextIter iter; 
    gtk_text_buffer_get_end_iter(txtbuffer, &iter);

    gtk_text_buffer_insert (
        txtbuffer,
        &iter,
        text,
        n
    );

    // scroll to bottom after we send a msg
    scrolledWindowToBottomImp(widgets.scroll);
}

static inline void output(char* buffer, int n){
    writeToChatWindow(buffer, n);
    memset(buffer, 0, n);
}

static void read_stdout(char *buffer){
    char c;
    unsigned int count = 0;
    while ( read(STDIN_FILENO, &c, 1) == 1) {
        buffer[count] = c;
        count++;
    }

    if( count > 0){
        output(buffer, count);
    }
}

// have to add g_module_export to our signal handlers otherwise they
// don't get put into our symbol table for gtk signals
// also need -rdynamic in make to export symbols so our function is visible
G_MODULE_EXPORT
void on_entry_activate(GtkEntry *entry, struct widget_refs *widgets)
{
    // send_text_to_overkill_daemon();
    // get text buffers
    
    GtkEntryBuffer* entrybuffer = gtk_entry_get_buffer(widgets->entry);
   
    // Insert a newline at the end of each message
    //GtkTextIter iter;
    int message_len;
    if ( (message_len = gtk_entry_buffer_get_length(entrybuffer)) ){
        gtk_entry_buffer_insert_text(entrybuffer, message_len, "\n", 1);
    }
    
    writeToChatWindow(
        gtk_entry_buffer_get_text(entrybuffer),
        gtk_entry_buffer_get_bytes(entrybuffer)
    );
    
    // write to stdout, chained program is linebuffered so don't forget the newline 
    write(STDOUT_FILENO, gtk_entry_buffer_get_text(entrybuffer), gtk_entry_buffer_get_bytes(entrybuffer));

    // clear
    gtk_entry_set_text(entry, "");
}

static inline void loadConfigFiles(){
    glade_file = getenv("gladefile");
    css_file = getenv("cssfile");

    if (!glade_file || !css_file){
        glade_file = GLADEFILEDEFAULT;
        css_file = CSSFILEDEFAULT;
    }
}


int main(int argc, char** argv)
{
    // Load necessary config files for gui
    loadConfigFiles();
    printf("glade file %s\n", glade_file);

    // Initialize main gtk window
    gtk_init(&argc, &argv);

    // Create a builder object that will load the file.
    GtkBuilder* builder = gtk_builder_new();

    // Load the XML from a file.
    gtk_builder_add_from_file(builder, glade_file, NULL);

    // try loading style from css file
    GtkCssProvider *provider = gtk_css_provider_new ();
    gtk_css_provider_load_from_path (provider,
        css_file, NULL);
    // have to add style context to the screen otherwise 
    gtk_style_context_add_provider_for_screen(gdk_screen_get_default(),
        GTK_STYLE_PROVIDER(provider),
        GTK_STYLE_PROVIDER_PRIORITY_USER);

    // connect struct members to named gtkWidgets
    widgets.log = GTK_TEXT_VIEW( gtk_builder_get_object(builder, "chatlog"));
    widgets.entry = GTK_ENTRY( gtk_builder_get_object(builder, "chatentry"));
    widgets.scroll = GTK_SCROLLED_WINDOW( gtk_builder_get_object(builder, "chatlogscroll"));
        
    // connect the signal handlers defined in the glade file.
    gtk_builder_connect_signals(builder, &widgets);

    // get the main-window object from glade file and show
    GObject* window = gtk_builder_get_object(builder, "main-window");
    gtk_widget_show(GTK_WIDGET(window));

    // quit when the main window is closed.
    g_signal_connect(window, "destroy", G_CALLBACK(gtk_main_quit), NULL);

    // set read from subprocess stdout to nonblocking
    int retval = fcntl( STDIN_FILENO, F_SETFL, fcntl(STDIN_FILENO, F_GETFL) | O_NONBLOCK);
    printf("Ret from fcntl: %d\n", retval);

    // Add subprocess read function to event loop
    char buffer[512];    
    g_idle_add_full(
        G_PRIORITY_DEFAULT_IDLE,
        G_SOURCE_FUNC(read_stdout),
        &buffer,
        NULL
    );

    // main loop.
    gtk_main();
	
    return 0;
}