package keypairs

import (
	"os"

	"github.com/aws/aws-sdk-go/aws"
	"github.com/aws/aws-sdk-go/aws/awserr"
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
	err := os.WriteFile(fileName, []byte(*fileData), 0400)
	return err
}


func SetupKeys(keyPairName string, keyStoreLocation string, profile string, region string) {
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
	createRes, err := CreateKeyPair(ec2Client, keyName)
	if err != nil {
		log.Error("Couldn't create key pair: %v", err)
		return
	}

	err = WriteKey(keyStoreLocation, createRes.KeyMaterial)
	if err != nil {
		log.Error("Couldn't write key pair to file: %v", err)
		return
	}
	log.Info("Created key pair: ", *createRes.KeyName)
}

func DeleteKeyPair(keyPair string, profile string, region string) {
	sess, err := session.NewSessionWithOptions(session.Options{
		Profile: profile,
		Config: aws.Config{
			Region: aws.String(region),
		},
	})

	if err != nil {
		log.Warn("Failed to initialize new session: %v", err)
		return
	}

	ec2Client := ec2.New(sess)
	_, err = ec2Client.DeleteKeyPair(&ec2.DeleteKeyPairInput{
        KeyName: aws.String(keyPair),
    })
    if err != nil {
        if aerr, ok := err.(awserr.Error); ok && aerr.Code() == "InvalidKeyPair.Duplicate" {
            log.Warn("Key pair %q does not exist.", keyPair)
        }
        log.Debug("Unable to delete key pair: %s, %v.", keyPair, err)
    }

    log.Info("Successfully deleted %q key pair\n", keyPair)
}
