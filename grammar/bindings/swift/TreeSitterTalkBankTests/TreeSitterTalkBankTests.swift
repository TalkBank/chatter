import XCTest
import SwiftTreeSitter
import TreeSitterTalkbank

final class TreeSitterTalkbankTests: XCTestCase {
    func testCanLoadGrammar() throws {
        let parser = Parser()
        let language = Language(language: tree_sitter_talkbank())
        XCTAssertNoThrow(try parser.setLanguage(language),
                         "Error loading TalkBank CHAT grammar")
    }
}
