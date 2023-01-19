package main

import (
	"bufio"
	"fmt"
	"os"
	"os/user"
	"sync"
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
	keyStoreLocation := "/home/" + currentUser.Username + "/.ssh/" + keyPairName + ".pem"
	profile, exist := os.LookupEnv("PROFILE")
	if !exist {
		log.Panic("Profile not set")
	}
	region, exist := os.LookupEnv("REGION")
	if !exist {
		log.Panic("Region not set")
	}

	keypairs.SetupKeys(keyPairName, keyStoreLocation, profile, region)
	var wg sync.WaitGroup;
	wg.Add(1)
	go create_ami(keyPairName, keyStoreLocation, profile, region, "x86")
	wg.Add(1)
	go create_ami(keyPairName, keyStoreLocation, profile, region, "amd")
	wg.Wait()
}

func create_ami(keyPairName string, keyStoreLocation string, profile string, region string, arch string) {
	log.Info("Creataing AMI for " + arch)
	
	newInstanceID := instances.LaunchInstance(keyPairName, profile, region, arch)
	time.Sleep(2 * time.Minute)
	instance := instances.GetInstanceDetails(*newInstanceID, profile, region)

	client := connect.NewSshClient(
		"ubuntu",
		*(instance.PublicIpAddress),
		22,
		keyStoreLocation,
	)
	SetupPreRequisites(client, *(instance.PublicIpAddress), *newInstanceID, profile, region)

	instances.CreateAMI(*newInstanceID, profile, region, arch)
	time.Sleep(7*time.Minute)
	TearDown(*newInstanceID, profile, region)
}


func SetupPreRequisites(client *connect.SshClient, host string, instanceID string, profile string, region string) {
	RunCommand(client, "sudo apt-get -y update")
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
	RunCommand(client, "sudo systemctl restart nitro-enclaves-allocator.service")
	RunCommand(client, "sudo systemctl enable nitro-enclaves-allocator.service")

	// proxies
	RunCommand(client, "wget -O vsock-to-ip-transparent http://public.artifacts.marlin.pro/projects/enclaves/vsock-to-ip-transparent_v1.0.0_linux_amd64")
	RunCommand(client, "chmod +x vsock-to-ip-transparent")
	RunCommand(client, "wget -O port-to-vsock-transparent http://public.artifacts.marlin.pro/projects/enclaves/port-to-vsock-transparent_v1.0.0_linux_amd64")
	RunCommand(client, "chmod +x port-to-vsock-transparent")

	// supervisord
	RunCommand(client, "sudo apt-get -y install supervisor")
	connect.TransferFile(client.Config, host, "./proxies.conf", "/home/ubuntu/proxies.conf")
	RunCommand(client, "sudo mv /home/ubuntu/proxies.conf /etc/supervisor/conf.d/proxies.conf")
	RunCommand(client, "sudo supervisorctl reload")
	
	RunCommand(client, "rm /home/ubuntu/allocator.yaml")
	RunCommand(client, "sudo rm -r /home/ubuntu/aws-nitro-enclaves-cli")
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
