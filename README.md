
# Setup AWS EC2 AMI's and VPC for oyster
Following is the description of the process to perform preliminary setups including setting up Base Amazon Machine Images and VPC to run a provider. The AMI's and VPC setup by this tutorial is used by oyster to run enclaves for jobs in EC2 instances.

This tutuorial assumes Ubuntu 20.4+. If you have an older Ubuntu or a different distro, commands might need modification before use.

 
## Preliminaries

### Setup AWS profiles using the AWS CLI
This setup requires you to setup a named profile using AWS CLI 

 - To install AWS CLI on your system please follow ["Installing or updating the latest version of the AWS CLI"](https://docs.aws.amazon.com/cli/latest/userguide/getting-started-install.html)
 - Next configure the AWS CLI and setup a named profile by following ["Configuring the AWS CLI"](https://docs.aws.amazon.com/cli/latest/userguide/cli-chap-configure.html)

### Install Go
This project requires Go version 1.18.1+ to run, to install go on your system, run the following command

    sudo apt install golang-go
You can then check the version by running 

    go version

 

### Pulumi preliminaries

## Setting up the VPC

## Setting up default Amazon Machine Images
### Step 1: Setup the repository
Clone the repository containing code base to run the setup by running the following commands

    git clone git@github.com:marlinprotocol/EnclaveLauncher.git && cd EnclaveLauncher
### Step 2: Build the executable
Run the following commands

    go get && go build
### Step 3: Run the executable
The executable requires a few environment variables to run, to set those up run the following commands. 

Set the name of the key pair to be used, if the specified key and, the *.pem* file in the `.ssh` folder in your home directory, exist, then in that case the existing keypair would be used otherwise a new keypair with the name specified would be created. 

    export KEY=/*keyname*/
    
Set the AWS profile and the region of setup

    export PROFILE=/*profile*/
    export REGION=/*region*/
Now to run the executable

    ./EnclaveLauncher
This process takes a while to run, where it creates the base EC2 instance and creates AMI's from them, and then proceeds to terminate the EC2 instances. At the end of this, you will have two AMI's by the name of  **`MarlinLauncherx86_64`** for `x86_64` architecture and **`MarlinLauncherARM64`** for `arm_64` architecture. Both AMI's will be tagged by **`project:oyster`** tag.

 

