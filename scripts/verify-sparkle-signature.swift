import CryptoKit
import Foundation

func fail(_ reason: String) -> Never {
  FileHandle.standardError.write(Data("\(reason)\n".utf8))
  exit(EXIT_FAILURE)
}

let arguments = CommandLine.arguments
guard arguments.count == 5 else {
  fail("Expected a DMG path, SHA-256 digest, Sparkle public key and signature.")
}
let whitespace = CharacterSet.whitespacesAndNewlines
guard let keyBytes = Data(base64Encoded: arguments[3].trimmingCharacters(in: whitespace)),
  let key = try? Curve25519.Signing.PublicKey(rawRepresentation: keyBytes)
else {
  fail("The bundle does not contain a valid Ed25519 public key.")
}
guard let signature = Data(base64Encoded: arguments[4].trimmingCharacters(in: whitespace)),
  signature.count == 64
else {
  fail("The appcast does not contain a valid Ed25519 signature.")
}
guard let diskImage = try? Data(contentsOf: URL(fileURLWithPath: arguments[1])) else {
  fail("Could not read the DMG for Sparkle verification.")
}
let digest = SHA256.hash(data: diskImage).map { String(format: "%02x", $0) }.joined()
guard digest == arguments[2] else {
  fail("The DMG bytes do not match the published SHA-256 digest.")
}
guard key.isValidSignature(signature, for: diskImage) else {
  fail("The Sparkle signature does not match the DMG and bundle public key.")
}
