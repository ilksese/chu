package main

import (
	"log"

	"github.com/chu-app/chu/internal/debugserver"
)

func main() {
	log.SetFlags(0)
	debugserver.Start("127.0.0.1:17321")
}
