package main

import (
	"log"

	"github.com/liudingchao/chu/internal/debugserver"
)

func main() {
	log.SetFlags(0)
	debugserver.Start("127.0.0.1:17321")
}
