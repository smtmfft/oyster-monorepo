package keypairs

import (
	"fmt"

	"github.com/aws/aws-sdk-go/aws"
	"github.com/aws/aws-sdk-go/aws/session"
	"github.com/aws/aws-sdk-go/service/ec2"
)

func GetKeyPairs(client *ec2.EC2) (*ec2.DescribeKeyPairsOutput, error) {
	result, err := client.DescribeKeyPairs(nil)
	if err != nil {
		return nil, err
	}

	return result, err
}


func DescribeKeyPairs(profile string, region string) {
	sess, err := session.NewSessionWithOptions(session.Options{
		Profile: profile,
		Config: aws.Config{
			Region: aws.String(region),
		},
	})

	if err != nil {
		fmt.Printf("Failed to initialize new session: %v", err)
		return
	}

	ec2Client := ec2.New(sess)

	keyPairRes, err := GetKeyPairs(ec2Client)
	if err != nil {
		fmt.Printf("Couldn't fetch key pairs: %v", err)
		return
	}

	fmt.Println("Key Pairs: ")
	for _, pair := range keyPairRes.KeyPairs {
		fmt.Printf("	%s \n ---- \n", *pair.KeyName)
	}
}

func CheckForKeyPair(keyPair string, profile string, region string) (bool) {
	sess, err := session.NewSessionWithOptions(session.Options{
		Profile: profile,
		Config: aws.Config{
			Region: aws.String(region),
		},
	})

	if err != nil {
		fmt.Printf("Failed to initialize new session: %v", err)
		return false
	}

	ec2Client := ec2.New(sess)

	keyPairRes, err := GetKeyPairs(ec2Client)
	if err != nil {
		fmt.Printf("Couldn't fetch key pairs: %v", err)
		return false
	}

	for _, pair := range keyPairRes.KeyPairs {
		if *pair.KeyName == keyPair {
			return true
		}
	}

	return false
}