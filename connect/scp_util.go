package connect

import (
	"bufio"
	"context"
	"fmt"
	"os"
	"time"

	scp "github.com/bramvdbogaerde/go-scp"
	log "github.com/sirupsen/logrus"
	"golang.org/x/crypto/ssh"
)

func TransferFile(clientConfig *ssh.ClientConfig, host string, fileSource string, fileDestination string) {
	curTime := time.Now()
	fmt.Println("=============================================================================================")
	log.Info("FILE TRANSFER: " + fileSource + " -> " + fileDestination)
	fmt.Println("")

	// Create a new SCP client
	client := scp.NewClient(host+":22", clientConfig)

	// Connect to the remote server
	err := client.Connect()
	if err != nil {
		log.Warn("Couldn't establish a connection to the remote server ", err)

		reader := bufio.NewReader(os.Stdin)
		fmt.Print("Retry? ")
		line, _ := reader.ReadString('\n')

		if line == "Y\n" || line == "yes\n" {
			TransferFile(clientConfig, host, fileSource, fileDestination)
		} else if line != "continue\n" {
			os.Exit(1)
		}

		return
	}

	// Open a file
	f, _ := os.Open(fileSource)

	// Close client connection after the file has been copied
	defer client.Close()

	// Close the file after it has been copied
	defer f.Close()

	// Finaly, copy the file over
	// Usage: CopyFile(fileReader, remotePath, permission)

	err = client.CopyFromFile(context.Background(), *f, fileDestination, "0655")
	dur := time.Now().Sub(curTime)
	log.Debug("Time : ", dur.Seconds())
	if err != nil {
		log.Warn("Error while copying file ", err)

		reader := bufio.NewReader(os.Stdin)
		fmt.Print("Retry? ")
		line, _ := reader.ReadString('\n')

		if line == "Y\n" || line == "yes\n" {
			TransferFile(clientConfig, host, fileSource, fileDestination)
		} else if line != "continue\n" {
			os.Exit(1)
		}
	}
}
