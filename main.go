package main

import (
	"bufio"
	"fmt"
	"os"
	"os/user"
	"time"

	"EnclaveLauncher/connect"
	"EnclaveLauncher/instances"
	"EnclaveLauncher/keypairs"

	log "github.com/sirupsen/logrus"
)

func main() {

	log.SetFormatter(&log.TextFormatter{
		FullTimestamp: false,
	})
	log.SetLevel(log.DebugLevel)
	

	keyPairName, exist := os.LookupEnv("KEY")
	if !exist {
		log.Panic("Key not set")
	} 
	currentUser, err := user.Current()
	if err != nil {
		log.Panic(err.Error())
	}
	keyStoreLocation := "/home/" + currentUser.Username + "/" + keyPairName + ".pem"
	profile, exist := os.LookupEnv("PROFILE")
	if !exist {
		log.Panic("Profile not set")
	} 
	region, exist := os.LookupEnv("REGION")
	if !exist {
		log.Panic("Region not set")
	}

	keypairs.SetupKeys(keyPairName, keyStoreLocation, profile, region)
	newInstanceID := instances.LaunchInstance(keyPairName, profile, region)
	time.Sleep(2 * time.Minute)
	instance := instances.GetInstanceDetails(*newInstanceID, profile, region)

	client := connect.NewSshClient(
		"ubuntu",
		*(instance.PublicIpAddress),
		22,
		keyStoreLocation,
	)
	SetupPreRequisites(client, *(instance.PublicIpAddress), *newInstanceID, profile, region)

	instances.CreateAMI(*newInstanceID, profile, region)
	TearDown(*newInstanceID, profile, region)
}


func SetupPreRequisites(client *connect.SshClient, host string, instanceID string, profile string, region string) {
	RunCommand(client, "sudo apt-get -y update")
	RunCommand(client, "sudo apt-get -y install sniproxy")
	RunCommand(client, "sudo service sniproxy start")
	RunCommand(client, "sudo apt-get -y install build-essential")
	RunCommand(client, "grep /boot/config-$(uname -r) -e NITRO_ENCLAVES")
	RunCommand(client, "sudo apt-get -y install linux-modules-extra-aws")
	RunCommand(client, "sudo apt-get -y install docker.io")
	RunCommand(client, "sudo systemctl start docker")
	RunCommand(client, "sudo systemctl enable docker")
	RunCommand(client, "sudo usermod -aG docker ubuntu")
	RunCommand(client, "git clone https://github.com/aws/aws-nitro-enclaves-cli.git")
	RunCommand(client, "cd aws-nitro-enclaves-cli && THIS_USER=\"$(whoami)\"")
	RunCommand(client, "cd aws-nitro-enclaves-cli && export NITRO_CLI_INSTALL_DIR=/")
	RunCommand(client, "cd aws-nitro-enclaves-cli && make nitro-cli")
	RunCommand(client, "cd aws-nitro-enclaves-cli && make vsock-proxy")
	RunCommand(client, `cd aws-nitro-enclaves-cli && 
						sudo make NITRO_CLI_INSTALL_DIR=/ install &&
						source /etc/profile.d/nitro-cli-env.sh && 
						echo source /etc/profile.d/nitro-cli-env.sh >> ~/.bashrc && 
						nitro-cli-config -i`)

	connect.TransferFile(client.Config, host, "./allocator.yaml", "allocator.yaml")

	
	RunCommand(client, "sudo systemctl start nitro-enclaves-allocator.service")
	RunCommand(client, "sudo cp allocator.yaml /etc/nitro_enclaves/allocator.yaml")
	instances.RebootInstance(instanceID, profile, region)
	time.Sleep(2 * time.Minute)
	RunCommand(client, "sudo systemctl start nitro-enclaves-allocator.service")
	RunCommand(client, "sudo systemctl enable nitro-enclaves-allocator.service")
}


func RunCommand(client *connect.SshClient, cmd string) (string) {
	fmt.Println("============================================================================================")
	log.Info(cmd)
	fmt.Println("")

	output, err := client.RunCommand(cmd)
	
	if err != nil {
		log.Warn("SSH run command error %v", err)
		
		reader := bufio.NewReader(os.Stdin)
		fmt.Print("Retry? ")
		line, _ := reader.ReadString('\n')

		if line == "Y\n" || line == "yes\n" {
			return RunCommand(client, cmd)
		} else if line != "continue\n" {
			os.Exit(1)
		}
	}
	return output
}

func TearDown(instanceID string, profile string, region string) {
	instances.TerminateInstance(instanceID, profile, region)
}