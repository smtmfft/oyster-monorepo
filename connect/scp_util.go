package connect

import (
	"context"
	"fmt"
	"os"

	scp "github.com/bramvdbogaerde/go-scp"
	"golang.org/x/crypto/ssh"
)

func TransferFile(clientConfig *ssh.ClientConfig, host string, fileSource string, fileDestination string) {
	fmt.Println("=============================================================================================")
	fmt.Println("FILE TRANSFER: " + fileSource + " -> " + fileDestination)
	fmt.Println("")
	
	// Create a new SCP client
	client := scp.NewClient(host + ":22", clientConfig)

	// Connect to the remote server
	err := client.Connect()
	if err != nil {
		fmt.Println("Couldn't establish a connection to the remote server ", err)
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

	if err != nil {
		fmt.Println("Error while copying file ", err)
	}
}