// probe.cc -- test-only native module built against Electron 43.8.3's ABI with the classic (non-N-API) V8 interface,
// so it loads only in an Electron-flavoured Node (NODE_MODULE_VERSION 148), never in plain Node 24 (137).
// Exports runtime(): a fixed string. No input, no I/O. Not part of the app.
#include <node.h>
namespace probe {
void Runtime(const v8::FunctionCallbackInfo<v8::Value>& args) {
  args.GetReturnValue().Set(v8::String::NewFromUtf8(args.GetIsolate(), "electron-abi-ok").ToLocalChecked());
}
void Init(v8::Local<v8::Object> exports) { NODE_SET_METHOD(exports, "runtime", Runtime); }
NODE_MODULE(NODE_GYP_MODULE_NAME, Init)
}
