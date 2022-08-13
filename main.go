package main

import (
	"bufio"
	"fmt"
	"os"
	"time"

	"EnclaveLauncher/connect"
	"EnclaveLauncher/instances"
	"EnclaveLauncher/keypairs"

	log "github.com/sirupsen/logrus"
)

func main() {
	// f, err := os.OpenFile("./testlogrus.json", os.O_APPEND | os.O_CREATE | os.O_RDWR, 0666)
    // if err != nil {
    //     fmt.Printf("error opening file: %v", err)
    // }
	// defer f.Close()
	log.SetFormatter(&log.TextFormatter{
		FullTimestamp: false,
	})
	// log.SetOutput(f)
	log.SetLevel(log.DebugLevel)
	// log.Info("HERE")
	// return
	startTime := time.Now()
	// fileAddress := "/home/nisarg/Desktop/startup.eif"
	// imageNameTag := "nitroimg:latest"
	keyPairName := "enclave-launcher"
	keyStoreLocation := "/home/nisarg/enclave-launcher.pem"
	profile := "marlin-one"
	region := "ap-south-1"

	keypairs.SetupKeys(keyPairName, keyStoreLocation, profile, region)
	newInstanceID := instances.LaunchInstance(keyPairName, profile, region)
	time.Sleep(2 * time.Minute)
	instance := instances.GetInstanceDetails(*newInstanceID, profile, region)
	// instance := instances.GetInstanceDetails("i-0e8ee6f053059f3a9", profile, region)
	log.Info("IP: ", *(instance.PublicIpAddress))

	client := connect.NewSshClient(
		"ubuntu",
		*(instance.PublicIpAddress),
		22,
		keyStoreLocation,
	)
	curTime := time.Now()
	// SetupPreRequisites(client, *(instance.PublicIpAddress), *newInstanceID, profile, region)
	// log.Debug("INSTANCE SETUP COMPLETE! :", time.Now().Sub(curTime).Seconds())
	// TransferAndLoadDockerImage(client, *(instance.PublicIpAddress), fileAddress, imageNameTag, "/home/ubuntu/docker_image.tar")
	// connect.TransferFile(client.Config, *(instance.PublicIpAddress), fileAddress, "/home/ubuntu/startup.eif")
	// log.Debug("DOCKER IMAGE SET UP!")
	// curTime = time.Now()
	BuildAndRunEnclave(client)
	log.Debug("IMAGE RUN : ", time.Now().Sub(curTime).Seconds())
	log.Debug("DONE IN: ", time.Now().Sub(startTime).Minutes())
}

func BuildAndRunEnclave(client *connect.SshClient) {
	// RunCommand(client, "nitro-cli build-enclave --docker-uri " + image + " --output-file startup.eif")
	RunCommand(client, "nitro-cli run-enclave --cpu-count 2 --memory 4500 --eif-path startup.eif --debug-mode")
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

func TransferAndLoadDockerImage(client *connect.SshClient, host string, file string, image string, destination string) {
	connect.TransferFile(client.Config, host, file, destination)

	RunCommand(client, "docker load < docker_image.tar")
	// RunCommand(client, "docker run " + image)
}

func RunCommand(client *connect.SshClient, cmd string) (string) {
	curTime := time.Now()
	fmt.Println("============================================================================================")
	log.Info(cmd)
	fmt.Println("")

	output, err := client.RunCommand(cmd)
	
	// fmt.Println(output)
	dur := time.Now().Sub(curTime)
	log.Debug("Time : ", dur.Seconds())
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