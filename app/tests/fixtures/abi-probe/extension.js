// extension.js -- test-only extension for phase I6d: loads a native module built for Electron's ABI (148) and writes
// one fixed line to the extension host log. In plain Node (ABI 137) the require throws and activation fails.
exports.activate = function () {
  const probe = require("./native/build/Release/probe.node");
  console.log("abi-probe: " + probe.runtime() + " modules=" + process.versions.modules);
};
