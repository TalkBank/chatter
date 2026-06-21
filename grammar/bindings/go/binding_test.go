package tree_sitter_talkbank_test

import (
	"testing"

	tree_sitter "github.com/tree-sitter/go-tree-sitter"
	tree_sitter_talkbank "github.com/TalkBank/tree-sitter-talkbank/bindings/go"
)

func TestCanLoadGrammar(t *testing.T) {
	language := tree_sitter.NewLanguage(tree_sitter_talkbank.Language())
	if language == nil {
		t.Errorf("Error loading TalkBank CHAT grammar")
	}
}
