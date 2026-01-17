#define XK_MISCELLANY
#define XK_LATIN1
#include <X11/keysymdef.h>
#include <X11/Xlib.h> // will include X11 which #defines None... Don't mess with order of includes.
#include <X11/Xutil.h>
#include <unistd.h> // syscall
#undef None
#include <filesystem>
#include <optional>

#include <Shared/Context.h>

static const char* s_applicationName  = "BabylonNative Playground";
static const char* s_applicationClass = "Playground";

namespace
{
    std::optional<Context> g_context{};

    void Uninitialize()
    {
        if (g_context)
        {
            g_context->DeviceUpdate().Finish();
            g_context->Device().FinishRenderingCurrentFrame();
        }

g_context.reset();
    }

    void InitBabylon(Window window, int width, int height, int argc, const char* const* argv)
    {
        Uninitialize();

        std::vector<std::string> scripts;

        if (argc == 0)
        {
            scripts.push_back("app:///Scripts/experience.js");
        }
        else
        {
            for (int i = 1; i < argc; ++i)
            {
                scripts.push_back(argv[i]);
            }

            scripts.push_back("app:///Scripts/playground_runner.js");
        }

        g_context.emplace(window, width, height, scripts);
    }

    void UpdateWindowSize(float width, float height)
    {
        if (g_context)
        {
            g_context->Device().UpdateSize(width, height);
        }
    }
}

int main(int _argc, const char* const* _argv)
{
    XInitThreads();
    Display* display = XOpenDisplay(NULL);

    int32_t screen = DefaultScreen(display);
    int32_t depth  = DefaultDepth(display, screen);
    Visual* visual = DefaultVisual(display, screen);
    Window root   = RootWindow(display, screen);
    const int width = 600;
    const int height = 400;

    XSetWindowAttributes windowAttrs;
    windowAttrs.background_pixel = 0;
    windowAttrs.background_pixmap = 0;
    windowAttrs.border_pixel = 0;
    windowAttrs.event_mask = 0
            | ButtonPressMask
            | ButtonReleaseMask
            | ExposureMask
            | KeyPressMask
            | KeyReleaseMask
            | PointerMotionMask
            | StructureNotifyMask
            ;

    Window window = XCreateWindow(display
                            , root
                            , 0, 0
                            , width, height, 0
                            , depth
                            , InputOutput
                            , visual
                            , CWBorderPixel|CWEventMask
                            , &windowAttrs
                            );

    // Clear window to black.
    XSetWindowAttributes attr;
    memset(&attr, 0, sizeof(attr) );
    XChangeWindowAttributes(display, window, CWBackPixel, &attr);

    const char* wmDeleteWindowName = "WM_DELETE_WINDOW";
    Atom wmDeleteWindow;
    XInternAtoms(display, (char **)&wmDeleteWindowName, 1, False, &wmDeleteWindow);
    XSetWMProtocols(display, window, &wmDeleteWindow, 1);

    XMapWindow(display, window);
    XStoreName(display, window, s_applicationName);

    XClassHint* hint = XAllocClassHint();
    hint->res_name  = const_cast<char*>(s_applicationName);
    hint->res_class = const_cast<char*>(s_applicationClass);
    XSetClassHint(display, window, hint);
    XFree(hint);

    XIM im = XOpenIM(display, NULL, NULL, NULL);

    XIC ic = XCreateIC(im
            , XNInputStyle
            , 0
            | XIMPreeditNothing
            | XIMStatusNothing
            , XNClientWindow
            , window
            , NULL
            );

    InitBabylon(window, width, height, _argc, _argv);
    UpdateWindowSize(width, height);

    bool exit{};
    while (!exit)
    {
        if (!XPending(display) && g_context)
        {
            g_context->DeviceUpdate().Finish();
            g_context->Device().FinishRenderingCurrentFrame();
            g_context->Device().StartRenderingCurrentFrame();
            g_context->DeviceUpdate().Start();
        }
        else
        {
            XEvent event;
            XNextEvent(display, &event);

            switch (event.type)
            {
                case Expose:
                    break;
                case ClientMessage:
                    if ( (Atom)event.xclient.data.l[0] == wmDeleteWindow)
                    {
                        Uninitialize();
                        exit = true;
                    }
                    break;
                case ConfigureNotify:
                    {
                        const XConfigureEvent& xev = event.xconfigure;
                        UpdateWindowSize(xev.width, xev.height);
                    }
                    break;
                case ButtonPress:
                    {
                        const XMotionEvent& xmotion = event.xmotion;
                        const XButtonEvent& xbutton = event.xbutton;

                        if (g_context) {
                            switch (xbutton.button) {
                                case Button1:
                                    g_context->Input().MouseDown(Babylon::Plugins::NativeInput::LEFT_MOUSE_BUTTON_ID, xmotion.x, xmotion.y);
                                    break;
                                case Button2:
                                    g_context->Input().MouseDown(Babylon::Plugins::NativeInput::MIDDLE_MOUSE_BUTTON_ID, xmotion.x, xmotion.y);
                                    break;
                                case Button3:
                                    g_context->Input().MouseDown(Babylon::Plugins::NativeInput::RIGHT_MOUSE_BUTTON_ID, xmotion.x, xmotion.y);
                                    break;
                                case Button4:
                                    g_context->Input().MouseWheel(Babylon::Plugins::NativeInput::MOUSEWHEEL_Y_ID, -120);
                                    break;
                                case Button5:
                                    g_context->Input().MouseWheel(Babylon::Plugins::NativeInput::MOUSEWHEEL_Y_ID, 120);
                                    break;
                            }
                        }
                    }
                    break;
                case ButtonRelease:
                    {
                        const XMotionEvent& xmotion = event.xmotion;
                        const XButtonEvent& xbutton = event.xbutton;

                        if (g_context)
                        {
                            switch (xbutton.button)
                            {
                                case Button1:
                                    g_context->Input().MouseUp(Babylon::Plugins::NativeInput::LEFT_MOUSE_BUTTON_ID, xmotion.x, xmotion.y);
                                    break;
                                case Button2:
                                    g_context->Input().MouseUp(Babylon::Plugins::NativeInput::MIDDLE_MOUSE_BUTTON_ID, xmotion.x, xmotion.y);
                                    break;
                                case Button3:
                                    g_context->Input().MouseUp(Babylon::Plugins::NativeInput::RIGHT_MOUSE_BUTTON_ID, xmotion.x, xmotion.y);
                                    break;
                            }
                        }
                    }
                    break;
                case MotionNotify:
                    {
                        const XMotionEvent& xmotion = event.xmotion;
                        if (g_context) {
                            g_context->Input().MouseMove(xmotion.x, xmotion.y);
                        }
                    }
                    break;
            }
        }
    }
    XDestroyIC(ic);
    XCloseIM(im);

    XUnmapWindow(display, window);
    XDestroyWindow(display, window);
    XCloseDisplay(display);
    return 0;
}
