package main

import (
	"fmt"
	"time"

	"EnclaveLauncher/connect"
	"EnclaveLauncher/instances"
	"EnclaveLauncher/keypairs"
)

func main() {
	fileAddress := "/home/nisarg/tbr.tar"
	imageNameTag := "nitroimg:latest"
	keyPairName := "auto-enclave"
	keyStoreLocation := "/home/nisarg/auto-enclave.pem"
	profile := "marlin-one"
	region := "ap-south-1"

	keypairs.SetupKeys(keyPairName, keyStoreLocation, profile, region)
	newInstanceID := instances.LaunchInstance(keyPairName, profile, region)
	time.Sleep(2 * time.Minute)
	instance := instances.GetInstanceDetails(*newInstanceID, profile, region)
	// instance := instances.GetInstanceDetails("i-0e8ee6f053059f3a9", profile, region)
	fmt.Println("IP: ", *(instance.PublicIpAddress))

	client, err := connect.NewSshClient(
		"ubuntu",
		*(instance.PublicIpAddress),
		22,
		keyStoreLocation,
	)
	if err != nil {
		fmt.Println("SSH Error: ", err)
	} else {
		SetupPreRequisites(client, *(instance.PublicIpAddress), *newInstanceID, profile, region)
		fmt.Println("SETUP COMPLETE!")
		TransferAndLoadDockerImage(client, *(instance.PublicIpAddress), fileAddress, imageNameTag, "/home/ubuntu/docker_image.tar")
		fmt.Println("DOCKER IMAGE SET UP!")
		BuildAndRunEnclave(client, imageNameTag)
	}
	fmt.Println("DONE!")
}

func BuildAndRunEnclave(client *connect.SshClient, image string) {
	RunCommand(client, "nitro-cli build-enclave --docker-uri " + image + " --output-file startup.eif")
	RunCommand(client, "nitro-cli run-enclave --cpu-count 2 --memory 4500 --eif-path startup.eif --debug-mode")
}

func SetupPreRequisites(client *connect.SshClient, host string, instanceID string, profile string, region string) {
	RunCommand(client, "sudo apt-get -y update")
	RunCommand(client, "sudo apt-get -y install sniproxy")
	RunCommand(client, "sudo service sniproxy start")
	RunCommand(client, "sudo usermod -aG ne ubuntu")
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
	RunCommand(client, "sudo systemctl enable nitro-enclaves-allocator.service")
}

func TransferAndLoadDockerImage(client *connect.SshClient, host string, file string, image string, destination string) {
	connect.TransferFile(client.Config, host, file, destination)

	RunCommand(client, "docker load < docker_image.tar")
	// RunCommand(client, "docker run " + image)
}

func RunCommand(client *connect.SshClient, cmd string) (string) {
	fmt.Println("============================================================================================")
	fmt.Println(cmd)
	fmt.Println("")

	output, err := client.RunCommand(cmd)
	
	fmt.Println(output)
	if err != nil {
		fmt.Printf("SSH run command error %v", err)
	}

	// reader := bufio.NewReader(os.Stdin)
	// fmt.Print("Enter text: ")
	// text, _ := reader.ReadString('\n')
	
	// if text == "STOP" {
	// 	os.Exit(1)
	// }
	return output
}