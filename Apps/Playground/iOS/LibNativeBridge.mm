#include "LibNativeBridge.h"

#import <Babylon/AppRuntime.h>
#import <Babylon/Graphics/Device.h>
#import <Babylon/ScriptLoader.h>
#import <Babylon/Plugins/NativeEngine.h>
#import <Babylon/Plugins/NativeInput.h>
#import <Babylon/Polyfills/Console.h>
#import <Babylon/Polyfills/Window.h>
#import <Babylon/Polyfills/XMLHttpRequest.h>
#import <Babylon/DebugTrace.h>
#import <optional>

std::optional<Babylon::Graphics::Device> device{};
std::optional<Babylon::Graphics::DeviceUpdate> update{};
std::optional<Babylon::AppRuntime> runtime{};
Babylon::Plugins::NativeInput* nativeInput{};
float screenScale{1.0f};

@implementation LibNativeBridge

- (instancetype)init
{
    self = [super init];
    return self;
}

- (void)dealloc
{
    if (device)
    {
        update->Finish();
        device->FinishRenderingCurrentFrame();
    }
    
    nativeInput = {};
    runtime.reset();
    update.reset();
    device.reset();
}

- (void)init:(MTKView*)view screenScale:(float)inScreenScale width:(int)inWidth height:(int)inHeight
{
    screenScale = inScreenScale;
    float width = inWidth;
    float height = inHeight;

    Babylon::DebugTrace::EnableDebugTrace(true);
    Babylon::DebugTrace::SetTraceOutput([](const char* trace) { NSLog(@"%s", trace); });

    Babylon::Graphics::Configuration graphicsConfig{};
    graphicsConfig.Window = view;
    graphicsConfig.Width = static_cast<size_t>(width);
    graphicsConfig.Height = static_cast<size_t>(height);

    device.emplace(graphicsConfig);
    update.emplace(device->GetUpdate("update"));

    device->StartRenderingCurrentFrame();
    update->Start();

    runtime.emplace();

    runtime->Dispatch([](Napi::Env env)
    {
        device->AddToJavaScript(env);

        Babylon::Polyfills::Console::Initialize(env, [](const char* message, auto) {
            NSLog(@"%s", message);
        });

        Babylon::Polyfills::Window::Initialize(env);

        Babylon::Polyfills::XMLHttpRequest::Initialize(env);

        Babylon::Plugins::NativeEngine::Initialize(env);

        nativeInput = &Babylon::Plugins::NativeInput::CreateForJavaScript(env);
    });

    Babylon::ScriptLoader loader{ *runtime };
    loader.LoadScript("app:///Scripts/babylon.max.js");
    loader.LoadScript("app:///Scripts/babylonjs.loaders.js");
    loader.LoadScript("app:///Scripts/experience.js");
}

- (void)resize:(int)inWidth height:(int)inHeight
{
    if (device)
    {
        update->Finish();
        device->FinishRenderingCurrentFrame();

        device->UpdateSize(static_cast<size_t>(inWidth), static_cast<size_t>(inHeight));

        device->StartRenderingCurrentFrame();
        update->Start();
    }
}

- (void)render
{
    if (device)
    {
        update->Finish();
        device->FinishRenderingCurrentFrame();
        device->StartRenderingCurrentFrame();
        update->Start();
    }
}

- (void)setTouchDown:(int)pointerId x:(int)inX y:(int)inY
{
    if (nativeInput != nullptr) {
        nativeInput->TouchDown(pointerId, inX * screenScale, inY * screenScale);
    }
}

- (void)setTouchMove:(int)pointerId x:(int)inX y:(int)inY
{
    if (nativeInput != nullptr) {
        nativeInput->TouchMove(pointerId, inX * screenScale, inY * screenScale);
    }
}

- (void)setTouchUp:(int)pointerId x:(int)inX y:(int)inY
{
    if (nativeInput != nullptr) {
        nativeInput->TouchUp(pointerId, inX * screenScale, inY * screenScale);
    }
}

@end
