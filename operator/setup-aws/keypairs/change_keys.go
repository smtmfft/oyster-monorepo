package keypairs

import (
	"bytes"
	"fmt"
	"os"
	"os/exec"

	"github.com/aws/aws-sdk-go/aws"
	"github.com/aws/aws-sdk-go/aws/session"
	"github.com/aws/aws-sdk-go/service/ec2"
	log "github.com/sirupsen/logrus"
)

func CreateKeyPair(client *ec2.EC2, keyName string) (*ec2.CreateKeyPairOutput, error) {
	result, err := client.CreateKeyPair(&ec2.CreateKeyPairInput{
		KeyName: aws.String(keyName),
	})

	if err != nil {
		return nil, err
	}

	return result, nil
}

func WriteKey(fileName string, fileData *string) error {
	err := os.WriteFile(fileName, []byte(*fileData), 0600)
	return err
}

func SetupKeys(keyPairName string, keyStoreLocation string, profile string, region string) {
	privateKeyLocation := keyStoreLocation + "/" + keyPairName + ".pem"
	publicKeyLocation := keyStoreLocation + "/" + keyPairName + ".pub"

	sess, err := session.NewSessionWithOptions(session.Options{
		Profile: profile,
		Config: aws.Config{
			Region: aws.String(region),
		},
	})

	if err != nil {
		log.Error("Failed to initialize new session: %v", err)
		return
	}

	ec2Client := ec2.New(sess)

	keyName := keyPairName
	keyExists := CheckForKeyPair(keyName, profile, region)
	_, err = os.Stat(privateKeyLocation)
	if err == nil && keyExists {
		return
	} else if os.IsNotExist(err) && !keyExists {
		createRes, err := CreateKeyPair(ec2Client, keyName)
		if err != nil {
			log.Error("Couldn't create key pair: %v", err)
			return
		}

		err = WriteKey(privateKeyLocation, createRes.KeyMaterial)
		if err != nil {
			log.Error("Couldn't write key pair to file: %v", err)
			return
		}
		log.Info("Created key pair: ", *createRes.KeyName)
	} else if err == nil && !keyExists {
		cmd := exec.Command("ssh-keygen", "-y", "-f", privateKeyLocation)
		var out bytes.Buffer
		var stderr bytes.Buffer
		cmd.Stdout = &out
		cmd.Stderr = &stderr
		err := cmd.Run()
		if err != nil {
			log.Error("key generation failed: ", err)
			log.Panic(fmt.Sprint(err) + ": " + stderr.String())
		}
		err = os.WriteFile(publicKeyLocation, []byte(out.Bytes()), 0600)
		if err != nil {
			log.Error("key generation failed: ", err)
		}
		importRes, err := ImportKeyPair(keyName, publicKeyLocation, profile, region)

		if err != nil {
			log.Panic(err)
		} else {
			log.Info("Created key pair: ", *importRes.KeyName)
		}
	} else {
		log.Panic("Key already exists, try with a different key name", err)
	}
}

func ImportKeyPair(keyPairName string, publicKeyLocation string, profile string, region string) (*ec2.ImportKeyPairOutput, error) {
	sess, err := session.NewSessionWithOptions(session.Options{
		Profile: profile,
		Config: aws.Config{
			Region: aws.String(region),
		},
	})

	if err != nil {
		return nil, err
	}

	ec2Client := ec2.New(sess)

	dat, err := os.ReadFile(publicKeyLocation)
	if err != nil {
		return nil, err
	}
	result, err := ec2Client.ImportKeyPair(&ec2.ImportKeyPairInput{
		KeyName:           aws.String(keyPairName),
		PublicKeyMaterial: dat,
	})

	return result, err
}
